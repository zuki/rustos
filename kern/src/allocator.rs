mod linked_list;
pub mod util;

mod bin;
mod bump;

type AllocatorImpl = bin::Allocator;

#[cfg(test)]
mod tests;

use core::alloc::{GlobalAlloc, Layout};
use core::fmt;

use crate::console::kprintln;
use crate::mutex::Mutex;
use pi::atags::{Atag, Atags};

/// `LocalAlloc`は標準ライブラリの `GlobalAlloc` に類似の
/// トレイトであるが `alloc()` と `dealloc()` で `&mut self` を取る.
pub trait LocalAlloc {
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8;
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout);
}

/// 特定のメモリアロケータをラップするスレッドセーフな（ロッキング）
/// ラッパー.
pub struct Allocator(Mutex<Option<AllocatorImpl>>);

impl Allocator {
    /// 初期化されていない `Allocator` を返す.
    ///
    /// アロケータは最初のメモリを割り当てる前に `initialize()` を
    /// 呼んで初期化しなければならない。これを怠るとパニックになる.
    pub const fn uninitialized() -> Self {
        Allocator(Mutex::new(None))
    }

    /// メモリアロケータを初期化する.
    /// callerはこのメソッドがカーネル初期化中に一度だけ
    /// 呼び出されることを保証しなければならない。
    ///
    /// # Panics
    ///
    /// システムのメモリマップを取り出せなかった場合はパニックになる.
    pub unsafe fn initialize(&self) {
        let (start, end) = memory_map().expect("failed to find memory map");
        //kprintln!("start: {}, end: {}", start, end);
        *self.0.lock() = Some(AllocatorImpl::new(start, end));
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .lock()
            .as_mut()
            .expect("allocator uninitialized")
            .dealloc(ptr, layout);
    }
}

extern "C" {
    static __text_end: u8;
}

/// 決定可能であればこのシステムで利用可能なメモリの
/// （開始アドレス, 終了アドレス）を返す. できない場合は
/// `None` を返す.
///
/// 通常の状態であればこの関数は`Some`を返すことが期待される。
pub fn memory_map() -> Option<(usize, usize)> {
    let _page_size = 1 << 12;
    let binary_end = unsafe { (&__text_end as *const u8) as usize };
    let mut end_addr: usize = 0;

    let mut atags = Atags::get();
    while let Some(atag) = atags.next() {
        if let Some(mem) = atag.mem() {
            end_addr = mem.start as usize + mem.size as usize;
            break;
        }
    }

    if end_addr == 0 || end_addr < binary_end {
        return None;
    }
    //kprintln!("binary_end: {}", binary_end);
    Some((binary_end, end_addr))
}


impl fmt::Debug for Allocator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.lock().as_mut() {
            Some(ref alloc) => write!(f, "{:?}", alloc)?,
            None => write!(f, "Not yet initialized")?,
        }
        Ok(())
    }
}
