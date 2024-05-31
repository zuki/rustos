pub mod sd;

use alloc::rc::Rc;
use core::fmt::{self, Debug};
use shim::io;
//use shim::ioerr;
use shim::path::Path;

pub use fat32::traits;
use fat32::vfat::{VFat, VFatHandle};

//use self::sd::Sd;
use crate::mutex::Mutex;

#[derive(Clone)]
pub struct PiVFatHandle(Rc<Mutex<VFat<Self>>>);

// これらの実装は *不健全* である。`PiVFatHandle`の `Sync`
// トレイトと `Send` トレイトの実装には `Rc` ではなく `Arc` を
// 使うべきである。しかし、`Arc` はアトミックメモリアクセスを
// 使用するため、ARMアーキテクチャではMMUを初期化する必要がある。
// 私たちはボード上の1つのコアしか有効にしていないので、これらの
// 不健全な実装は今のところ直ちに害を及ぼすことはない。いずれ
// 修正する予定である。
unsafe impl Send for PiVFatHandle {}
unsafe impl Sync for PiVFatHandle {}

impl Debug for PiVFatHandle {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "PiVFatHandle")
    }
}

impl VFatHandle for PiVFatHandle {
    fn new(val: VFat<PiVFatHandle>) -> Self {
        PiVFatHandle(Rc::new(Mutex::new(val)))
    }

    fn lock<R>(&self, f: impl FnOnce(&mut VFat<PiVFatHandle>) -> R) -> R {
        f(&mut self.0.lock())
    }
}

pub struct FileSystem(Mutex<Option<PiVFatHandle>>);

impl FileSystem {
    /// 初期化していない `FileSystem` を返す.
    ///
    /// 最初のメモリ割り当てを行う前に `initialize()` を呼び出して
    /// ファイルシステムを初期化する必要がある。そうしないと
    /// パニックを起こすことになる。
    pub const fn uninitialized() -> Self {
        FileSystem(Mutex::new(None))
    }

    /// ファイルシステムを初期化する.
    ///
    /// callerはカーネルの初期化の際に1度だけこのメソッドを
    /// 実行するいつ用がある。
    ///
    /// # パニック
    ///
    /// ディスクまたはファイルシステムの初期化に失敗した場合は
    /// パニックを起こす
    pub unsafe fn initialize(&self) {
        let sd = sd::Sd::new().expect("failed to init sd card");
        let vfat = VFat::<PiVFatHandle>::from(sd).expect("failed to init vfat");

        *self.0.lock() = Some(vfat);
    }
}

// FIXME: Implement `fat32::traits::FileSystem` for `&FileSystem`

impl fat32::traits::FileSystem for &FileSystem {
    type File = fat32::vfat::File<PiVFatHandle>;
    type Dir = fat32::vfat::Dir<PiVFatHandle>;
    type Entry = fat32::vfat::Entry<PiVFatHandle>;

    fn open<P: AsRef<Path>>(self, path: P) -> io::Result<Self::Entry> {
        self.0.lock().as_ref().expect("kernel::fs uninitialized").open(path)
    }
}
