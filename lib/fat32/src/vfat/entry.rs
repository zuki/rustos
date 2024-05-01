use crate::traits;
use crate::vfat::{Dir, File, Metadata, VFatHandle};
use core::fmt;

// You can change this definition if you want
#[derive(Debug)]
pub enum Entry<HANDLE: VFatHandle> {
    File(File<HANDLE>),
    Dir(Dir<HANDLE>),
}

impl<HANDLE: VFatHandle> traits::Entry for Entry<HANDLE> {
    type File = File<HANDLE>;
    type Dir = Dir<HANDLE>;
    type Metadata = Metadata;

    // FIXME: Implement `traits::Entry` for `Entry`.
    /// このエントリに対応するファイルまたはディレクトリの名前.
    fn name(&self) -> &str {
        match self {
            Entry::File(f) => f.name.as_str(),
            Entry::Dir(d) => d.name.as_str(),
        }
    }

    /// このエントリに関連付けられたメタデータ.
    fn metadata(&self) -> &Self::Metadata {
        match self {
            Entry::File(f) => &f.metadata,
            Entry::Dir(d) => &d.metadata,
        }
    }

    /// `self` がファイルの場合はファイルへの参照の `Some` を返す。
    /// それでなければ `None` を返す。
    fn as_file(&self) -> Option<&File<HANDLE>> {
        match self {
            Entry::File(f) => Some(f),
            Entry::Dir(_) => None,
        }
    }

    /// `self` がディレクトリの場合はディレクトリへの参照の `Some` を返す。
    /// それでなければ `None` を返す。
    fn as_dir(&self) -> Option<&Dir<HANDLE>> {
        match self {
            Entry::File(_) => None,
            Entry::Dir(d) => Some(d),
        }
    }

    /// `self` がファイルの場合はファイルの `Some` を返す。
    /// それでなければ `None` を返す。
    fn into_file(self) -> Option<File<HANDLE>> {
        match self {
            Entry::File(f) => Some(f),
            Entry::Dir(_) => None,
        }
    }

    /// `self` がディレクトリの場合はディレクトリの `Some` を返す。
    /// それでなければ `None` を返す。
    fn into_dir(self) -> Option<Dir<HANDLE>> {
        match self {
            Entry::File(_) => None,
            Entry::Dir(d) => Some(d),
        }
    }
}
