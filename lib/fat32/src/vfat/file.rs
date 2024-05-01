use alloc::string::String;

use shim::io::{self, SeekFrom};
use shim::ioerr;

use crate::traits;
use crate::vfat::{Cluster, Metadata, VFatHandle};
use crate::vfat::vfat::SeekHandle;

#[derive(Debug)]
pub struct File<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
    pub size: u32,
    pointer: SeekHandle,
}

impl<HANDLE: VFatHandle> File<HANDLE> {
    pub fn new(vfat: HANDLE, cluster: Cluster, name: String,
        metadata: Metadata, size: u32) -> File<HANDLE> {
        File {
            vfat,
            cluster,
            name,
            metadata,
            size,
            pointer: SeekHandle {
                cluster,
                offset: 0,
                total_offset: 0,
            }
        }
    }
}

// FIXME: Implement `traits::File` (and its supertraits) for `File`.
impl<HANDLE: VFatHandle> traits::File for File<HANDLE> {
    /// バッファされているデータをディスクに書き出す.
    fn sync(&mut self) -> io::Result<()> {
        unimplemented!("read only file system")
    }

    /// ファイルのバイト単位のサイズを返す.
    fn size(&self) -> u64 {
        self.size as u64
    }
}

impl<HANDLE: VFatHandle> io::Write for File<HANDLE> {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        unimplemented!("read only file system")
    }

    fn flush(&mut self) -> io::Result<()> {
        unimplemented!("read only file system")
    }
}

impl<HANDLE: VFatHandle> io::Read for File<HANDLE> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pointer.total_offset >= self.size as usize {
            return Ok(0);
        }

        let max_file_read = core::cmp::min(
            (self.size as usize) - self.pointer.total_offset, buf.len());

        let (written, cloff) = self.vfat.lock(|fs|
            fs.read_cluster_unaligned(self.pointer, &mut buf[..max_file_read]))?;
            self.pointer = cloff;
            Ok(written)
    }
}

impl<HANDLE: VFatHandle> io::Seek for File<HANDLE> {
    /// ファイルのオフセット `pos` にシークする.
    ///
    /// ファイルの末尾へのシークを可能にする。ファイル終端を
    /// _超える_ シークは エラー `InvalidInput` を返す。
    ///
    /// シーク操作が成功したら、このメソッドはストリームの開始点からの
    /// 新しい位置を返す。以後、この位置は  SeekFrom::Start を使って
    /// 使用することができる。
    ///
    /// # エラー
    ///
    /// ファイルの開始点の前、または終端の後ろにシークしようとすると
    /// エラー `InvalidInput` となる。
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        // クロージャ
        let f = |offset| self.vfat.lock(|fs| {
            if offset < 0 {
                return ioerr!(InvalidInput, "cannot seek befor start of file");
            }

            fs.seek_handle(self.cluster, self.pointer, offset as usize)
        });

        let cloff = match pos {
            SeekFrom::Start(start) => f(start as isize)?,
            SeekFrom::End(end) => f(self.size as isize + end as isize)?,
            SeekFrom::Current(current) => f(self.pointer.total_offset as isize
                + current as isize)?,
        };

        self.pointer = cloff;
        Ok(cloff.total_offset as u64)
    }
}
