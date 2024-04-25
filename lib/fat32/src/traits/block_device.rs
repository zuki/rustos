use alloc::boxed::Box;
use alloc::vec::Vec;
use shim::io;

/// セクタ粒度で読み書きできるデバイスにより実装されるトレイト.
pub trait BlockDevice: Send {
    /// バイト単位のセクタサイズ。512バイト以上で512の倍数でなければ
    /// ならない。デフォルトは512.
    fn sector_size(&self) -> u64 {
        512
    }

    /// セクタ番号 `n` を `buf` に読み込む.
    ///
    /// `self.sector_size()` または `buf.len()` の小さい方のバイトを
    /// `buf` に読み込む。読み込んだバイト数が返される。
    ///
    /// # エラー
    ///
    /// `self` のシークまたは読み込みに失敗した場合はエラーを返す。
    fn read_sector(&mut self, n: u64, buf: &mut [u8]) -> io::Result<usize>;

    /// セクタ番号 `n` を `vec` に追加する.
    ///
    /// `self.sector_size()` バイトが `vec` に追加される。読み込んだ
    /// バイト数が返される。
    ///
    /// # エラー
    ///
    /// `self` のシークまたは読み込みに失敗した場合はエラーを返す。
    fn read_all_sector(&mut self, n: u64, vec: &mut Vec<u8>) -> io::Result<usize> {
        let sector_size = self.sector_size() as usize;

        let start = vec.len();
        vec.reserve(sector_size);

        unsafe {
            vec.set_len(start + sector_size);
        }
        // XXX. handle: clean-up dirty data when failed
        let read = self.read_sector(n, &mut vec[start..])?;
        unsafe {
            vec.set_len(start + read);
        }
        Ok(read)
    }

    /// `buf` の内容でセクタ `n` を上書きする.
    ///
    /// `self.sector_size()` または `buf.len()` の小さい方のバイトを
    /// セクタに書き出す。書き出したバイト数が返される。
    ///
    /// # エラー
    ///
    /// `self` のシークまたは読み込みに失敗した場合はエラーを返す。
    /// `buf` の長さが `self.sector_size()` より短い場合、
    /// `UnexpectedEof` エラーを返す。
    fn write_sector(&mut self, n: u64, buf: &[u8]) -> io::Result<usize>;
}

impl<'a, T: BlockDevice> BlockDevice for &'a mut T {
    fn read_sector(&mut self, n: u64, buf: &mut [u8]) -> io::Result<usize> {
        (*self).read_sector(n, buf)
    }

    fn write_sector(&mut self, n: u64, buf: &[u8]) -> io::Result<usize> {
        (*self).write_sector(n, buf)
    }
}

macro impl_for_read_write_seek($(<$($gen:tt),*>)* $T:path) {
    use shim::io::{Read, Write, Seek};

    impl $(<$($gen),*>)* BlockDevice for $T {
        fn read_sector(&mut self, n: u64, buf: &mut [u8]) -> io::Result<usize> {
            let sector_size = self.sector_size();
            let to_read = ::core::cmp::min(sector_size as usize, buf.len());
            self.seek(io::SeekFrom::Start(n * sector_size))?;
            self.read_exact(&mut buf[..to_read])?;
            Ok(to_read)
        }

        fn write_sector(&mut self, n: u64, buf: &[u8]) -> io::Result<usize> {
            let sector_size = self.sector_size();
            let to_write = ::core::cmp::min(sector_size as usize, buf.len());
            self.seek(io::SeekFrom::Start(n * sector_size))?;
            self.write_all(&buf[..to_write])?;
            Ok(to_write)
        }
    }
}

impl_for_read_write_seek!(<'a> shim::io::Cursor<&'a mut [u8]>);
impl_for_read_write_seek!(shim::io::Cursor<Vec<u8>>);
impl_for_read_write_seek!(shim::io::Cursor<Box<[u8]>>);
#[cfg(test)]
impl_for_read_write_seek!(::std::fs::File);
