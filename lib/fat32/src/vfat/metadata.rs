use core::fmt;

use alloc::string::String;

use crate::traits;

/// FAT32オンディスクにおいて表現される日付の構造体.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Date(u16);

/// FAT32オンディスクにおいて表現される時間の構造体.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Time(u16);

/// FAT32オンディスクにおいて表現されるファイル属性構造体.
#[repr(C, packed)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Attributes(u8);

/// 日付と時間を含む構造体.
#[derive(Default, Copy, Clone, Debug, PartialEq, Eq)]
pub struct Timestamp {
    pub date: Date,
    pub time: Time,
}

/// ディレクトリエントリのためのメタデータ.
#[derive(Default, Debug, Clone)]
pub struct Metadata {
    // FIXME: Fill me in.
    pub attributes: Attributes,
    pub created: Timestamp,
    pub accessed: Timestamp,
    pub modified: Timestamp,
}

impl Attributes {
    pub fn read_only(&self) -> bool {
        (self.0 & 0x01) != 0
    }

    pub fn hidden(&self) -> bool {
        (self.0 & 0x02) != 0
    }

    pub fn system(&self) -> bool {
        (self.0 & 0x04) != 0
    }

    pub fn volume_id(&self) -> bool {
        (self.0 & 0x08) != 0
    }

    pub fn directory(&self) -> bool {
        (self.0 & 0x10) != 0
    }

    pub fn archive(&self) -> bool {
        (self.0 & 0x20) != 0
    }
}
// FIXME: Implement `traits::Timestamp` for `Timestamp`.
impl traits::Timestamp for Timestamp {
    fn year(&self) -> usize {
        (((self.date.0 >> 9) & 0x7F) + 1980) as usize
    }

    fn month(&self) -> u8 {
        ((self.date.0 >> 5) & 0x0F) as u8
    }

    fn day(&self) -> u8 {
        (self.date.0 & 0x1F) as u8
    }

    fn hour(&self) -> u8 {
        ((self.time.0 >> 11) & 0x1F) as u8
    }

    fn minute(&self) -> u8 {
        ((self.time.0 >> 5) & 0x3F) as u8
    }

    fn second(&self) -> u8 {
        ((self.time.0 & 0x1F) * 2) as u8
    }
 }

// FIXME: Implement `traits::Metadata` for `Metadata`.
impl traits::Metadata for Metadata {
    type Timestamp = Timestamp;

    fn read_only(&self) -> bool {
        self.attributes.read_only()
    }

    fn hidden(&self) -> bool {
        self.attributes.hidden()
    }

    fn created(&self) -> Self::Timestamp {
        self.created
    }

    fn accessed(&self) -> Self::Timestamp {
        self.accessed
    }

    fn modified(&self) -> Self::Timestamp {
        self.modified
    }
}

// FIXME: Implement `fmt::Display` (to your liking) for `Metadata`.
impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use traits::Timestamp;
        f.write_fmt(format_args!("{:0>4}-{:0>2}-{:0>2} {:0>2}:{:0>2}:{:0>2}",
            self.year(), self.month(), self.day(),
            self.hour(), self.minute(), self.second()))
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use traits:: Metadata;
        f.debug_struct("Metadata")
            .field("read_only", &self.read_only())
            .field("hidden", &self.hidden())
            .field("created", &self.created)
            .field("accessed", &self.accessed)
            .field("modified", &self.modified)
            .finish()
    }
}
