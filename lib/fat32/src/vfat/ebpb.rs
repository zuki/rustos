use core::fmt;
use shim::const_assert_size;

use crate::traits::BlockDevice;
use crate::vfat::Error;

#[repr(C, packed)]
pub struct BiosParameterBlock {
    // FIXME: Fill me in.
    _nop: [u8; 3],
    pub oem_identifier: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub max_directory_entries: u16,
    total_logical_sectors: u16,
    pub fat_id: u8,
    sectors_per_fat: u16,
    pub sectors_per_track: u16,
    pub head_count: u16,
    pub hidden_sector_count: u32,
    total_logical_sectors_32: u32,
    // Extedn fields
    sectors_per_fat_32: u32,
    pub flags: u16,
    pub fat_version: u16,
    pub root_cluster: u32,
    pub fsinfo_sector: u16,
    pub backup_boot_sector: u16,
    _reserved_1: [u8; 12],
    pub drive_number: u8,
    _reserved_2: u8,
    pub signature: u8,
    pub volume_id: [u8; 4],
    pub volume_label: [u8; 11],
    pub system_identifier_string: [u8; 8],
    pub boot_code: [u8; 420],
    boot_signature: [u8; 2],
}

const_assert_size!(BiosParameterBlock, 512);

impl BiosParameterBlock {
    /// デバイス `device` のセクタ `sector` から FAT32拡張BIOS
    /// パラメータブロックを読み込む`.
    ///
    /// # エラー
    ///
    /// EBPBシグネチャが不正な場合、`BadSignature` エラーを返す。
    pub fn from<T: BlockDevice>(mut device: T, sector: u64) -> Result<BiosParameterBlock, Error> {
        let mut block = [0_u8; 512];
        device.read_sector(sector, &mut block).map_err(|e| Error::Io(e))?;

        let ebpb: BiosParameterBlock = unsafe { core::mem::transmute(block) };
        //if ebpb.signature != 0x28 && ebpb.signature != 0x29 {
        if ebpb.boot_signature != [0x55_u8, 0xAA_u8] {
            return Err(Error::BadSignature);
        }
        Ok(ebpb)
    }

    pub fn sectors_per_fat(&self) -> u32 {
        if self.sectors_per_fat != 0 {
            self.sectors_per_fat as u32
        } else {
            self.sectors_per_fat_32
        }
    }

    pub fn total_logical_sectores(&self) -> u32 {
        if self.total_logical_sectors != 0 {
            self.total_logical_sectors as u32
        } else {
            self.total_logical_sectors_32
        }
    }
}



impl fmt::Debug for BiosParameterBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BiosParameterBlock")
            .field("oem_identifier", &format_args!("{:?}", self.oem_identifier))
            .field("bytes_per_sector", &format_args!("{:?}", { self.bytes_per_sector }))
            .field("sectors_per_cluster", &format_args!("{:?}", self.sectors_per_cluster))
            .field("reserved_sectors", &format_args!("{:?}", { self.reserved_sectors }))
            .field("fat_count", &format_args!("{:?}", self.fat_count))
            .field("max_directory_entries", &format_args!("{:?}", { self.max_directory_entries }))
            .field("total_logical_sectors", &format_args!("{:?}", { self.total_logical_sectors }))
            .field("fat_id", &format_args!("{:?}", self.fat_id))
            .field("sectors_per_fat", &format_args!("{:?}", { self.sectors_per_fat }))
            .field("sectors_per_track", &format_args!("{:?}", { self.sectors_per_track }))
            .field("head_count", &format_args!("{:?}", { self.head_count }))
            .field("hidden_sector_count", &format_args!("{:?}", { self.hidden_sector_count }))
            .field("total_logical_sectors_32", &format_args!("{:?}", { self.total_logical_sectors_32 }))
            .field("sectors_per_fat_32", &format_args!("{:?}", { self.sectors_per_fat_32 }))
            .field("flags", &format_args!("{:?}", { self.flags }))
            .field("fat_version", &format_args!("{:?}", { self.fat_version }))
            .field("root_cluster", &format_args!("{:?}", { self.root_cluster }))
            .field("fsinfo_sector", &format_args!("{:?}", { self.fsinfo_sector }))
            .field("backup_boot_sector", &format_args!("{:?}", { self.backup_boot_sector }))
            .field("drive_number", &format_args!("{:?}", self.drive_number))
            .field("signature", &format_args!("{:?}", self.signature))
            .field("volume_id", &format_args!("{:?}", self.volume_id))
            .field("volume_label", &format_args!("{:?}", self.volume_label))
            .field("system_identifier_string", &format_args!("{:?}", self.system_identifier_string))
            .field("boot_signature", &format_args!("{:?}", self.boot_signature))
            .finish()
    }
}
