use core::fmt::Debug;
use core::marker::PhantomData;
//use core::mem::size_of;

use alloc::vec::Vec;

use shim::io;
use shim::ioerr;
//use shim::newioerr;
//use shim::path;
use shim::path::{Component, Path};

use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
//use crate::util::SliceExt;
use crate::vfat::{BiosParameterBlock, CachedPartition, Partition};
use crate::vfat::{Cluster, Dir, Entry, Error, FatEntry, File, Status};

/// クロージャとしてクリティカルセクションを処理するジェネリックトレイト
pub trait VFatHandle: Clone + Debug + Send + Sync {
    fn new(val: VFat<Self>) -> Self;
    fn lock<R>(&self, f: impl FnOnce(&mut VFat<Self>) -> R) -> R;
}

#[derive(Debug)]
pub struct VFat<HANDLE: VFatHandle> {
    phantom: PhantomData<HANDLE>,
    device: CachedPartition,
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    sectors_per_fat: u32,
    fat_start_sector: u64,
    data_start_sector: u64,
    rootdir_cluster: Cluster,
}

#[derive(Copy, Clone, Debug)]
pub struct SeekHandle {
    pub cluster: Cluster,
    pub offset: usize,
    pub total_offset: usize,
}

impl<HANDLE: VFatHandle> VFat<HANDLE> {
    pub fn from<T>(mut device: T) -> Result<HANDLE, Error>
    where
        T: BlockDevice + 'static,
    {
        let mbr = MasterBootRecord::from(&mut device)?;
        for partition in mbr.partitions.iter() {
            if partition.partition_type == 0xB || partition.partition_type == 0xC {
                let bpb = BiosParameterBlock::from(&mut device, partition.relative_sector as u64)?;
                return Ok(HANDLE::new(VFat {
                    phantom: PhantomData {},
                    device: CachedPartition::new(device, Partition {
                        start: partition.relative_sector as u64,
                        num_sectors: partition.total_sectores as u64,
                        sector_size: bpb.bytes_per_sector as u64,
                    }),
                    bytes_per_sector: bpb.bytes_per_sector,
                    sectors_per_cluster: bpb.sectors_per_cluster,
                    sectors_per_fat: bpb.sectors_per_fat(),
                    fat_start_sector: bpb.reserved_sectors as u64,
                    data_start_sector: (bpb.reserved_sectors as u64)
                        + (bpb.fat_count as u64 * bpb.sectors_per_fat() as u64),
                    rootdir_cluster: Cluster::from(bpb.root_cluster),
                }));
            }
        }
        Err(Error::NotFound)
    }

    /// クラスタ `cluster` の開始セクタを返す
    fn cluster_start(&self, cluster: Cluster) -> u64 {
        self.data_start_sector + (cluster.raw() as u64 - 2) * self.sectors_per_cluster as u64
    }

    /// クラスタのバイト単位のサイズを返す
    fn cluster_size_bytes(&self) -> usize {
        self.bytes_per_sector as usize * self.sectors_per_cluster as usize
    }

    // TODO: ここでは次のメソッドが役に立つだろう:
    //
    // * クラスタのオフセットからバッファに読み込む.
    //
    //   fn read_cluster(
    //       &mut self,
    //       cluster: Cluster,
    //       offset: usize,
    //       buf: &mut [u8]
    //   ) -> io::Result<usize>;
    //
    // * 開始クラスタのからクラスタチェーンをすべてベクタに読み込む
    //
    //   fn read_chain(
    //       &mut self,
    //        start: Cluster,
    //        buf: &mut Vec<u8>
    //   ) -> io::Result<usize>;
    //
    // * クラスタの `FatEntry` への参照を返す.
    //   この参照は直接キャッシュされたセクタを指している.
    //
    //    fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry>;


    /// クラスタのオフセットからバッファに読み込む.
    fn read_cluster(&mut self, cluster: Cluster, offset: usize, buf: &mut [u8]) -> io::Result<usize> {
        if offset >= self.cluster_size_bytes() {
            return Ok(0);
        }

        let cluster_start = self.cluster_start(cluster);
        let cluster_end = cluster_start + self.sectors_per_cluster as u64;

        let mut current_cluster = cluster_start + (offset / self.bytes_per_sector as usize) as u64;
        let mut offset = offset % self.bytes_per_sector as usize;

        let mut ptr: usize = 0;
        'sector_loop: while ptr < buf.len() && current_cluster < cluster_end {
            let sector_data = &self.device.get(current_cluster)?[offset..];
            offset = 0;

            for byte in sector_data.iter() {
                if ptr >= buf.len() {
                    break 'sector_loop;
                }
                buf[ptr] = *byte;
                ptr += 1;
            }

            current_cluster += 1;
        }

        Ok(ptr)
    }

    /// クラスタのオフセット位置から読み込む
    pub fn read_cluster_unaligned(&mut self, mut cloff: SeekHandle,
        buf: &mut [u8]) -> io::Result<(usize, SeekHandle)> {
        let mut written = 0_usize;

        'cluster_loop: while written < buf.len() {
            let bytes = self.read_cluster(cloff.cluster, cloff.offset,
                &mut buf[written..])?;
            written += bytes;
            cloff.offset += bytes;
            cloff.total_offset += bytes;

            if cloff.offset == self.cluster_size_bytes() {
                match self.fat_entry(cloff.cluster)?.status() {
                    Status::Data(next) => cloff = SeekHandle {
                        cluster: next,
                        offset: 0,
                        total_offset: cloff.total_offset,
                    },
                    Status::Eoc(_) => break 'cluster_loop,
                    _ => return ioerr!(Other, "unxepectted fat entry"),
                }
            }
        }

        Ok((written, cloff))

    }
    /// 開始クラスタのからクラスタチェーンをすべてベクタに読み込む,
    pub fn read_chain(&mut self, start: Cluster, buf: &mut Vec<u8>
        ) -> io::Result<usize> {
        let mut cluster = start;
        let initial_size = buf.len();

        'cluster_loop: loop {
            let start = buf.len();
            buf.resize(start + self.cluster_size_bytes(), 0);
            let wrote = self.read_cluster(cluster, 0, &mut buf.as_mut_slice()[start..])?;
            buf.truncate(start + wrote);

            match self.fat_entry(cluster)?.status() {
                Status::Data(next) => cluster = next,
                Status::Eoc(_) => break 'cluster_loop,
                _ => return ioerr!(Other, "unexpected fat entry"),
            }
        }

        Ok(buf.len() - initial_size)
    }

    /// クラスタの `FatEntry` への参照を返す.
    /// この参照は直接キャッシュされたセクタを指している.
    pub fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
        let entry_location = (self.fat_start_sector * self.bytes_per_sector as u64) + (cluster.raw() * 4) as u64;

        let sector = self.device.get(entry_location / self.bytes_per_sector as u64)?;

        let raw_entry = &sector[(entry_location % self.bytes_per_sector as u64) as usize];

        Ok(unsafe { core::mem::transmute::<&u8, &FatEntry>(raw_entry) })
    }

    /// ルートディレクトリのクラスタを返す
    pub fn root_cluster(&self) -> Cluster {
        self.rootdir_cluster
    }

    pub fn seek_handle(&mut self, start: Cluster, cloff: SeekHandle,
        offset: usize) -> io::Result<SeekHandle> {
        let mut current_cluster: Cluster;
        let mut current_offset: usize;

        if offset > cloff.total_offset {
            current_cluster = cloff.cluster;
            current_offset = offset - cloff.total_offset;
        } else {
            current_cluster = start;
            current_offset = offset;
        }

        'cluster_loop: while current_offset >= self.cluster_size_bytes() {
            match self.fat_entry(current_cluster)?.status() {
                Status::Data(next) => current_cluster = next,
                Status::Eoc(_) => break 'cluster_loop,
                _ => return ioerr!(Other, "unexpected fat entry"),
            }
            current_offset -= self.cluster_size_bytes();
        }

        Ok(SeekHandle {
            cluster: current_cluster,
            offset: current_offset,
            total_offset: offset,
        })
    }
}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Entry = Entry<HANDLE>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        use crate::traits::Entry as TraitEntry;

        let mut entry: Entry<HANDLE> = Entry::Dir(Dir::root(self.clone()));

        for component in path.as_ref().components() {
            match component {
                Component::RootDir => entry = Entry::Dir(Dir::root(self.clone())),
                Component::Normal(s) => match entry.as_dir() {
                    Some(d) => entry = d.find(s)?,
                    None => return ioerr!(PermissionDenied, "found file in path traversal"),
                }
                _ => return ioerr!(InvalidInput, "unexpected path item"),
            }
        }

        Ok(entry)
    }
}
