use shim::io;

use crate::traits::{Dir, Entry, File, Metadata, Timestamp};

/// ファイルシステムトレイトのすべてを実装する型.
#[derive(Copy, Clone)]
pub struct Dummy;

impl io::Write for Dummy {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        panic!("Dummy")
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("Dummy")
    }
}

impl io::Read for Dummy {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        panic!("Dummy")
    }
}

impl io::Seek for Dummy {
    fn seek(&mut self, _pos: io::SeekFrom) -> io::Result<u64> {
        panic!("Dummy")
    }
}

impl File for Dummy {
    fn sync(&mut self) -> io::Result<()> {
        panic!("Dummy")
    }
    fn size(&self) -> u64 {
        panic!("Dummy")
    }
}

/// ファイルシステムのディレクトリにより実装されるトレイト.
impl Dir for Dummy {
    /// このディレクトリに格納されるエントリの型.
    type Entry = Dummy;
    type Iter = Dummy;

    /// このディレクトリのすべてのエントリを走査するイテレータを返す.
    fn entries(&self) -> io::Result<Self::Iter> {
        panic!("Dummy")
    }
}

impl Iterator for Dummy {
    type Item = Dummy;
    fn next(&mut self) -> Option<Self::Item> {
        panic!("Dummy")
    }
}

impl Entry for Dummy {
    type File = Dummy;
    type Dir = Dummy;
    type Metadata = Dummy;

    fn name(&self) -> &str {
        panic!("Dummy")
    }
    fn metadata(&self) -> &Self::Metadata {
        panic!("Dummy")
    }
    fn as_file(&self) -> Option<&Self::File> {
        panic!("Dummy")
    }
    fn as_dir(&self) -> Option<&Self::Dir> {
        panic!("Dummy")
    }
    fn into_file(self) -> Option<Self::File> {
        panic!("Dummy")
    }
    fn into_dir(self) -> Option<Self::Dir> {
        panic!("Dummy")
    }
}

impl Timestamp for Dummy {
    fn year(&self) -> usize {
        panic!("Dummy")
    }
    fn month(&self) -> u8 {
        panic!("Dummy")
    }
    fn day(&self) -> u8 {
        panic!("Dummy")
    }
    fn hour(&self) -> u8 {
        panic!("Dummy")
    }
    fn minute(&self) -> u8 {
        panic!("Dummy")
    }
    fn second(&self) -> u8 {
        panic!("Dummy")
    }
}

impl Metadata for Dummy {
    type Timestamp = Dummy;
    fn read_only(&self) -> bool {
        panic!("Dummy")
    }
    fn hidden(&self) -> bool {
        panic!("Dummy")
    }
    fn created(&self) -> Self::Timestamp {
        panic!("Dummy")
    }
    fn accessed(&self) -> Self::Timestamp {
        panic!("Dummy")
    }
    fn modified(&self) -> Self::Timestamp {
        panic!("Dummy")
    }
}
