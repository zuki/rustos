use crate::vfat::*;
use core::fmt;

use self::Status::*;

#[derive(Debug, PartialEq)]
pub enum Status {
    /// FATエントリは未使用（空）のクラスタに対応する.
    Free,
    /// FATエントリ/クラスタは予約済み.
    Reserved,
    /// FATエントリは有効なクラスタに対応する.チェーンの
    /// 次のクラスタは`Cluster`.
    Data(Cluster),
    /// FATエントリは不良（ディスク障害）のクラスタに対応する
    Bad,
    /// FATエントリは有効なクラスタに対応する. 対象のクラスタは
    /// チェーンの最後のクラスタ.
    Eoc(u32),
}

#[repr(C, packed)]
pub struct FatEntry(pub u32);

impl FatEntry {
    /// FATエントリ `self` の `Status` を返す..
    pub fn status(&self) -> Status {
        match self.0 & 0x0FFF_FFFF {
            0x0 => Status::Free,
            0x1 => Status::Reserved,
            0x2..=0xFFF_FFEF => Status::Data(Cluster::from(self.0)),
            0xFFF_FFF0..=0x0FFF_FFF6 => Status::Reserved,
            0xFFF_FFF7 => Status::Bad,
            0xFFF_FFF8..=0xFFF_FFFF => Status::Eoc(self.0),
            _ => unreachable!(),
        }
    }
}

impl fmt::Debug for FatEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("FatEntry")
            .field("value", &{ self.0 })
            .field("status", &self.status())
            .finish()
    }
}
