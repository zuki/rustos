use core::fmt::Debug;
use core::marker::PhantomData;
use core::mem::size_of;

use alloc::vec::Vec;

use shim::io;
use shim::ioerr;
use shim::newioerr;
use shim::path;
use shim::path::Path;

use crate::mbr::MasterBootRecord;
use crate::traits::{BlockDevice, FileSystem};
use crate::util::SliceExt;
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

    // TODO: ここでは次のメソッドが役に立つだろう:
    //
    // クラスタのオフセットからバッファに読み込む.
    //fn read_cluster(&mut self, cluster: Cluster,
    //                 offset: usize, buf: &mut [u8]) -> io::Result<usize> {
    //}
    //
    // 開始クラスタのからクラスタチェーンをすべてベクタに読み込む
    //fn read_chain(&mut self, start: Cluster, buf: &mut Vec<u8>)
    //    -> io::Result<usize> {
    //
    //}
    //
    // クラスタの `FatEntry` への参照を返す.
    // この参照は直接キャッシュされたセクタを指している.
    //fn fat_entry(&mut self, cluster: Cluster) -> io::Result<&FatEntry> {
    //
    //}

}

impl<'a, HANDLE: VFatHandle> FileSystem for &'a HANDLE {
    type File = crate::traits::Dummy;
    type Dir = crate::traits::Dummy;
    type Entry = crate::traits::Dummy;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        unimplemented!("FileSystem::open()")
    }
}
