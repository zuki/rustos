use alloc::string::String;
use alloc::vec::Vec;

use shim::const_assert_size;
use shim::ffi::OsStr;
use shim::io;
use shim::ioerr;

use crate::traits;
use crate::util::VecExt;
use crate::vfat::{Attributes, Date, Metadata, Time, Timestamp};
use crate::vfat::{Cluster, Entry, File, VFatHandle};

#[derive(Debug)]
pub struct Dir<HANDLE: VFatHandle> {
    pub vfat: HANDLE,
    pub cluster: Cluster,
    pub name: String,
    pub metadata: Metadata,
}

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatRegularDirEntry {
    // FIXME: Fill me in.
    name: [u8; 8],
    ext: [u8; 3],
    attributes: Attributes,
    __reserve: u8,
    creation_time_tenth: u8,
    creation_time: Time,
    creation_date: Date,
    accessed_date: Date,
    cluster_high: u16,
    modified_time: Time,
    modified_date: Date,
    cluster_low: u16,
    file_size: u32
}

const_assert_size!(VFatRegularDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatLfnDirEntry {
    // FIXME: Fill me in.
    sequence_number: u8,
    name_set_1: [u8; 10],
    attributes: u8,
    lfn_type: u8,
    name_checksum: u8,
    name_set_2: [u8; 12],
    __reserve: u16,
    name_set_3: [u8; 4],
}

const_assert_size!(VFatLfnDirEntry, 32);

#[repr(C, packed)]
#[derive(Copy, Clone)]
pub struct VFatUnknownDirEntry {
    // FIXME: Fill me in.
    id: u8,
    __rserved_0: [u8; 10],
    attributes: u8,
    __reserved_1: [u8; 20],
}

//const_assert_size!(VFatUnknownDirEntry, 32);

pub union VFatDirEntry {
    unknown: VFatUnknownDirEntry,
    regular: VFatRegularDirEntry,
    long_filename: VFatLfnDirEntry,
}

impl VFatDirEntry {
    /// LFNエントリか
    pub fn is_lfn(&self) -> bool {
        (unsafe { self.unknown.attributes } == 0x0F )
    }

    /// 削除済み/未使用か
    pub fn is_deleted(&self) -> bool {
        let id = unsafe { self.unknown.id };
        id == 0xE5
    }

    /// ディレクトリの終わりか
    pub fn was_prev_last(&self) -> bool {
        let id = unsafe { self.unknown.id };
        id == 0
    }
}

impl VFatRegularDirEntry {
    pub fn metadata(&self) -> Metadata {
        Metadata {
            attributes: self.attributes,
            created: Timestamp {
                date: self.creation_date,
                time: self.creation_time,
            },
            accessed: Timestamp {
                date: self.accessed_date,
                time: Default::default(),
            },
            modified: Timestamp {
                date: self.modified_date,
                time: self.modified_time,
            },
        }
    }

    pub fn cluster(&self) -> Cluster {
        Cluster::from((self.cluster_high as u32) << 16 | self.cluster_low as u32)
    }

    pub fn basic_name(&self) -> String {
        let mut s = String::new();
        for c in self.name.iter().take_while(|c| ![b'\0', b' '].contains(c)) {
            s.push((*c).into());
        }
        let mut added_dot = false;
        for c in self.ext.iter().take_while(|c| ![b'\0', b' '].contains(c)) {
            if !added_dot {
                s.push('.');
                added_dot = true;
            }
            s.push((*c).into())
        }
        s
    }
}

impl VFatLfnDirEntry {
    pub fn sequence_number(&self) -> u8 {
        self.sequence_number & 0b1_1111
    }
}

impl<HANDLE: VFatHandle> Dir<HANDLE> {
    /// ルートディレクトリを返す
    pub fn root(vfat: HANDLE) -> Dir<HANDLE> {
        let cluster = vfat.lock(|fs| fs.root_cluster());

        Dir {
            vfat: vfat.clone(),
            cluster,
            name: String::from("/"),
            metadata: Default::default(),
        }
    }

    /// `self` からエントリ名 `name` を見つけてそれを返す.
    /// 比較は大文字小文字を区別しない。
    ///
    /// # エラー
    ///
    /// `self` にエントリ名 `name` がない場合はエラー `NotFound` を
    /// 返す。
    ///
    /// `name` に不正なUTF-8文字が含まれている場合はエラー
    /// `InvalidInput` を返す。
    pub fn find<P: AsRef<OsStr>>(&self, name: P) -> io::Result<Entry<HANDLE>> {
        use crate::traits::{Dir, Entry};

        let name = name.as_ref().to_str().ok_or(io::ErrorKind::InvalidInput)?;

        for entry in self.entries()? {
            if str::eq_ignore_ascii_case(entry.name(), name) {
                return Ok(entry);
            }
        }

        ioerr!(NotFound, "file not found")
    }
}

pub struct EntriesIterator<HANDLE: VFatHandle> {
    vfat: HANDLE,
    buf: Vec<VFatDirEntry>,
    index: usize,
}

fn parse_lfns(lfns: &mut Vec<VFatLfnDirEntry>) -> String {
    lfns.sort_by(|a, b| a.sequence_number().cmp(&b.sequence_number()));

    let mut buf: Vec<u8> = Vec::new();
    for lfn in lfns.iter() {
        for c in lfn.name_set_1.iter()
            .chain(lfn.name_set_2.iter())
            .chain(lfn.name_set_3.iter()) {
                buf.push(*c);
            }
    }

    let mut chars: Vec<u16> = Vec::new();
    for slice in buf.chunks(2) {
        let value = (slice[0] as u16) | ((slice[1] as u16) << 8);
        if value == 0 || value ==0xFFFF {
            break;
        }
        chars.push(value);
    }
    String::from_utf16_lossy(chars.as_slice())
}

impl<HANDLE: VFatHandle> Iterator for EntriesIterator<HANDLE> {
    type Item = Entry<HANDLE>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut lfns: Vec<VFatLfnDirEntry> = Vec::new();

        while self.index < self.buf.len() {
            let entry = &self.buf[self.index];

            if entry.was_prev_last(){
                return None;
            }

            self.index += 1;

            if entry.is_deleted() {
                continue;
            }

            // LFNエントリは複数ありうる
            if entry.is_lfn() {
                lfns.push(unsafe { entry.long_filename });
                continue;
            }
            // 通常エントリは1つでLFNに隣接する
            let entry = unsafe { entry.regular };

            let name: String;
            if lfns.len() > 0 {
                name = parse_lfns(&mut lfns);
                lfns.clear();
            } else {
                name = entry.basic_name();
            }

            return Some(if entry.attributes.directory() {
                Entry::Dir(Dir {
                    vfat: self.vfat.clone(),
                    cluster: entry.cluster(),
                    name,
                    metadata: entry.metadata(),
                })
            } else {
                Entry::File(File::new(
                    self.vfat.clone(),
                    entry.cluster(),
                    name,
                    entry.metadata(),
                    entry.file_size,
                ))
            });
        }
        None
    }
}

impl<HANDLE: VFatHandle> traits::Dir for Dir<HANDLE> {
    // FIXME: Implement `trait::Dir` for `Dir`.
    type Entry = Entry<HANDLE>;
    type Iter = EntriesIterator<HANDLE>;

    /// このディレクトリのエントリを走査するイテレータを返す.
    fn entries(&self) -> io::Result<Self::Iter> {
        let mut buf: Vec<u8> = Vec::new();

        self.vfat.lock(|fs| fs.read_chain(self.cluster, &mut buf))?;

        Ok(EntriesIterator {
            vfat: self.vfat.clone(),
            buf: unsafe { buf.cast() },
            index: 0,
        })
    }
}
