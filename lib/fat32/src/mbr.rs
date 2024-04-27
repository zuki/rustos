use core::fmt;
use shim::const_assert_size;
use shim::io;

use crate::traits::BlockDevice;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CHS {
    // FIXME: Fill me in.
    bytes: [u8; 3],
    // 8 bits - ヘッド
    // 6 bits - セクタ  (ビット7-8はシリンダの上位2ビット)
    // 10 bits - シリンダ
}

impl CHS {
    fn cylinder(&self) -> u16 {
        (self.bytes[2] as u16) | ((self.bytes[1] & 0b1100_0000_u8) as u16) << 2
    }

    fn head(&self) -> u8 {
        self.bytes[0]
    }

    fn sector(&self) -> u8 {
        self.bytes[1] & 0b0011_1111_u8
    }
}

// FIXME: implement Debug for CHS
impl fmt::Debug for CHS {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CHS")
            .field("cylinder", &self.cylinder())
            .field("head", &self.head())
            .field("sector", &self.sector())
            .finish()
    }
}

const_assert_size!(CHS, 3);

#[repr(C, packed)]
pub struct PartitionEntry {
    // FIXME: Fill me in.
    pub boot_indicator: u8,
    pub start: CHS,
    pub partition_type: u8,
    pub end: CHS,
    pub relative_sector: u32,
    pub total_sectores: u32,
}

// FIXME: implement Debug for PartitionEntry
impl fmt::Debug for PartitionEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CHS")
            .field("boot_indicator", &format_args!("{:?}", self.boot_indicator))
            .field("start", &format_args!("{:?}", self.start))
            .field("partition_type", &format_args!("{:?}", self.partition_type))
            .field("end", &format_args!("{:?}", self.end))
            .field("relative_sector", &{ self.relative_sector })
            .field("total_sectores", &{ self.total_sectores })
            .finish()
    }
}

const_assert_size!(PartitionEntry, 16);

/// マスターブートレコード (MBR).
#[repr(C, packed)]
pub struct MasterBootRecord {
    // FIXME: Fill me in.
    bootstrap: [u8; 436],
    pub disk_id: [u8; 10],
    pub partitions: [PartitionEntry; 4],
    signature: [u8; 2],
}

// FIXME: implemente Debug for MaterBootRecord
impl fmt::Debug for MasterBootRecord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("CHS")
            .field("disk_id", &format_args!("{:?}", self.disk_id))
            .field("partitions", &format_args!("{:?}", self.partitions))
            .finish()
    }
}

const_assert_size!(MasterBootRecord, 512);

#[derive(Debug)]
pub enum Error {
    /// MBRの読み込む中にI/Oエラーがあった.
    Io(io::Error),
    /// パーティション `.0` (0-indexed) は無効または未知のブート
    /// インジケータを含んでいる.
    UnknownBootIndicator(u8),
    /// MBRマジックシグネチャが無効.
    BadSignature,
}

impl MasterBootRecord {
    /// `device` からマスターブートレコードを読み込んで返す.
    ///
    /// # エラー
    ///
    /// MBRが無効なマジックシグネチャを含む場合、`BadSignature` を返す。
    /// パーティション `n` が無効または未知のブートインジケータを含んで
    /// いる場合、`UnknownBootIndicator(n)` を返す。
    /// MBRの読み込み中にI/Oエラー `err` が発生した場合、`Io(err)`を返す。
    pub fn from<T: BlockDevice>(mut device: T) -> Result<MasterBootRecord, Error> {
        let mut sector0 = [0_u8; 512];
        device.read_sector(0, &mut sector0).map_err(|e| Error::Io(e))?;

        let mbr: MasterBootRecord = unsafe { core::mem::transmute(sector0) };

        if mbr.signature != [0x55_u8, 0xAA_u8] {
            return Err(Error::BadSignature);
        }

        for (i, partition) in mbr.partitions.iter().enumerate() {
            if partition.boot_indicator != 0 && partition.boot_indicator != 0x80 {
                return Err(Error::UnknownBootIndicator(i as u8));
            }
        }

        Ok(mbr)
    }
}
