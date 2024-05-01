use shim::{io, path::Path};

use crate::traits::Metadata;

/// ファイルシステムのファイルにより実装されるトレイト.
pub trait File: io::Read + io::Write + io::Seek + Sized {
    /// バッファされているデータをディスクに書き出す.
    fn sync(&mut self) -> io::Result<()>;

    /// ファイルのバイト単位のサイズを返す.
    fn size(&self) -> u64;
}

/// ファイルシステムのディレクトリにより実装されるトレイト.
pub trait Dir: Sized {
    /// このディレクトリに格納されるエントリの型.
    type Entry: Entry;

    /// このディレクトリのエントリを走査するイテレータの型.
    type Iter: Iterator<Item = Self::Entry>;

    /// このディレクトリのエントリを走査するイテレータを返す.
    fn entries(&self) -> io::Result<Self::Iter>;
}

/// ファイルシステムのディレクトリエントリにより実装されるトレイト.
///
/// エントリは `File` か `Directory` のいずれかであり、`Metadata` と
/// 名前に関連付けられる。
pub trait Entry: Sized {
    type File: File;
    type Dir: Dir;
    type Metadata: Metadata;

    /// このエントリに対応するファイルまたはディレクトリの名前.
    fn name(&self) -> &str;

    /// このエントリに関連付けられたメタデータ.
    fn metadata(&self) -> &Self::Metadata;

    /// `self` がファイルの場合はファイルへの参照の `Some` を返す。
    /// それでなければ `None` を返す。
    fn as_file(&self) -> Option<&Self::File>;

    /// `self` がディレクトリの場合はディレクトリへの参照の `Some` を返す。
    /// それでなければ `None` を返す。
    fn as_dir(&self) -> Option<&Self::Dir>;

    /// `self` がファイルの場合はファイルの `Some` を返す。
    /// それでなければ `None` を返す。
    fn into_file(self) -> Option<Self::File>;

    /// `self` がディレクトリの場合はディレクトリの `Some` を返す。
    /// それでなければ `None` を返す。
    fn into_dir(self) -> Option<Self::Dir>;

    /// このエントリがファイルの場合は `true` を返す。そうでなければ
    /// `false` を返す。
    fn is_file(&self) -> bool {
        self.as_file().is_some()
    }

    /// このエントリがディレクトリの場合は `true` を返す。そうでなければ
    /// `false` を返す。
    fn is_dir(&self) -> bool {
        self.as_dir().is_some()
    }
}

/// ファイルシステムにより実装されるトレイト.
pub trait FileSystem: Sized {
    /// このファイルシステムのファイルの型.
    type File: File;

    /// このファイルシステムのディレクトリの型.
    type Dir: Dir<Entry = Self::Entry>;

    /// このファイルシステムのディレクトリエントリの型.
    type Entry: Entry<File = Self::File, Dir = Self::Dir>;

    /// `path` にあるエントリをオープンする. `path` は絶対パスで
    /// なければならない。
    ///
    /// # エラー
    ///
    /// If `path` が絶対パスでない場合、`InvalidInput` という種類の
    /// エラーが返される。
    ///
    /// `path` の最後のコンポーネント以外が既存のディレクトリを参照して
    /// いない場合、 `InvalidInput` という種類のエラーが返される。
    ///
    /// `path` にエントリがない場合、`NotFound` という種類のエラーが
    /// 返される。
    ///
    /// それ以外のエラー値はすべて実装定義である。
    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry>;

    /// `path` にあるファイルをオープンする. `path` は絶対パスで
    /// なければならない。
    ///
    /// # エラー
    ///
    /// `open()` のエラー条件に加え, この関数は `path` にあるエントリが
    /// 通常ファイルでない場合、`Other` という種類のエラーが返される。
    fn open_file<P: AsRef<Path>>(self, path: P) -> io::Result<Self::File> {
        self.open(path)?
            .into_file()
            .ok_or(io::Error::new(io::ErrorKind::Other, "not a regular file"))
    }

    /// `path` にあるディレクトリをオープンする. `path` は絶対パスで
    /// なければならない。
    ///
    /// # エラー
    ///
    /// `open()` のエラー条件に加え, この関数は `path` にあるエントリが
    /// ディレクトリでない場合、`Other` という種類のエラーが返される。
    fn open_dir<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Dir> {
        self.open(path)?
            .into_dir()
            .ok_or(io::Error::new(io::ErrorKind::Other, "not a directory"))
    }
}
