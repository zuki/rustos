use alloc::boxed::Box;
use alloc::vec::Vec;
use core::fmt;
use hashbrown::HashMap;
use shim::io;
use crate::vfat::Error;

use crate::traits::BlockDevice;

#[derive(Debug)]
struct CacheEntry {
    data: Vec<u8>,
    dirty: bool,
}

pub struct Partition {
    /// このパーティションが始まる物理セクタ.
    pub start: u64,
    /// セクタ数
    pub num_sectors: u64,
    /// パーティションの論理セクタのバイト単位のサイズ.
    pub sector_size: u64,
}

pub struct CachedPartition {
    device: Box<dyn BlockDevice>,
    cache: HashMap<u64, CacheEntry>,
    partition: Partition,
}

impl CachedPartition {
    /// `device` からセクタを透過的にキャッシュし、物理セクタを
    /// `partition` 内の論理セクタにマップする新しい `CachedPartition` を
    /// 作成する。`CacheDevice` からの読み込みと書き出しはすべて
    /// インメモリキャッシュで行われる。
    ///
    /// `partition` パラメータは論理セクタのサイズと論理セクタの開始位置を
    /// 決定する。セクタ番号 `0` へのアクセスは物理セクタ `partition.start`
    /// に変換される。セクタ番号 `[0, num_sectors)` の仮想セクタは
    /// アクセス可能である。
    ///
    /// `partition.sector_size` は `device.sector_size()` の整数倍で
    /// なければならない。
    ///
    /// # パニック
    ///
    /// パーティションのセクタサイズがデバイスのセクタサイズより小さい
    /// 場合はパニックになる。
    pub fn new<T>(device: T, partition: Partition) -> CachedPartition
    where
        T: BlockDevice + 'static,
    {
        assert!(partition.sector_size >= device.sector_size());

        CachedPartition {
            device: Box::new(device),
            cache: HashMap::new(),
            partition: partition,
        }
    }

    /// 論理セクタあたりの物理セクタ数を返すr.
    fn factor(&self) -> u64 {
        self.partition.sector_size / self.device.sector_size()
    }

    /// ユーザからのセクタ `virt` に対する要求を物理セクタにマップする。
    /// 仮想セクタ番号が範囲外の場合は `Nnone` を返す。
    fn virtual_to_physical(&self, virt: u64) -> Option<u64> {
        if virt >= self.partition.num_sectors {
            return None;
        }

        let physical_offset = virt * self.factor();
        let physical_sector = self.partition.start + physical_offset;

        Some(physical_sector)
    }

    fn load_sector(&mut self, buf: &mut Vec<u8>, sector: u64) -> io::Result<()> {
        buf.clear();
        buf.reserve(self.partition.sector_size as usize);

        let physical_sector = self.virtual_to_physical(sector)
            .ok_or(io::ErrorKind::InvalidInput)?;

        for i in 0..self.factor() {
            let mut raw = [0_u8; 512];
            self.device.read_sector(physical_sector + i, &mut raw)?;

            for c in raw.iter() {
                buf.push(*c);
            }
        }
        Ok(())
    }

    fn get_entry(&mut self, sector: u64) -> io::Result<&mut CacheEntry> {
        if let None = self.cache.get_mut(&sector) {
            let mut buf: Vec<u8> = Vec::new();
            self.load_sector(&mut buf, sector);

            self.cache.insert(sector, CacheEntry {
                dirty: false,
                data: buf,
            });
        }

        Ok(self.cache.get_mut(&sector).unwrap())
    }

    /// キャッシュされたセクタ `sector` への可変参照を返す。
    /// そのセクタがまだキャッシュされていない場合はまずディスクから
    /// そのセクタが読み込まれる。
    ///
    /// このメソッドを呼び出すとそのセクタはダーティとみなされる。
    /// セクタに書き込まれることが前提となっているからである。これを
    /// 意図しない場合は `get()` を使用すること。
    ///
    /// # エラー
    ///
    /// セクタをディスクから読み込む際にエラーが発生した場合はエラーを
    /// 返す。
    pub fn get_mut(&mut self, sector: u64) -> io::Result<&mut [u8]> {
        self.get_entry(sector).map(|entry| {
            entry.dirty = true;
            entry.data.as_mut_slice()
        })
    }

    /// キャッシュされたセクタ `sector` への参照を返す。そのセクタが
    /// まだキャッシュされていない場合はまずディスクからそのセクタが
    /// 読み込まれる。
    ///
    /// # エラー
    ///
    /// セクタをディスクから読み込む際にエラーが発生した場合はエラーを
    /// 返す。
    pub fn get(&mut self, sector: u64) -> io::Result<&[u8]> {
        self.get_entry(sector).map(|entry| entry.data.as_slice())
    }
}

// FIXME: Implement `BlockDevice` for `CacheDevice`. The `read_sector` and
// `write_sector` methods should only read/write from/to cached sectors.
impl BlockDevice for CachedPartition {
    fn sector_size(&self) -> u64 {
        self.partition.sector_size
    }

    fn read_sector(&mut self, sector: u64, buf: &mut [u8]) -> io::Result<usize> {
        match self.get(sector) {
            Ok(read_sector) => {
                let amt = core::cmp::min(read_sector.len(), buf.len());
                buf[..amt].clone_from_slice(&read_sector[..amt]);
                Ok(amt)
            }
            Err(e) =>Err(e),
        }
    }

    fn write_sector(&mut self, sector: u64, buf: &[u8]) -> io::Result<usize> {
        match self.get_mut(sector) {
            Ok(write_to_sector) => {
                let amt = core::cmp::min(write_to_sector.len(), buf.len());
                write_to_sector[..amt].clone_from_slice(&buf[..amt]);
                Ok(amt)
            }
            Err(e) =>Err(e),
        }
    }
}

impl fmt::Debug for CachedPartition {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CachedPartition")
            .field("device", &"<block device>")
            .field("cache", &self.cache)
            .finish()
    }
}
