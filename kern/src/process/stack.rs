use core::alloc::{GlobalAlloc, Layout};
use core::fmt;
use core::ptr::Unique;

use crate::vm::PhysicalAddr;
use crate::ALLOCATOR;

/// プロセスのスタック. デフォルトサイズは1MiBで16バイトアライン.
pub struct Stack {
    ptr: Unique<[u8; Stack::SIZE]>,
}

impl Stack {
    /// デフォルトのスタックサイズは1MiB.
    pub const SIZE: usize = 1 << 20;

    /// デフォルトのスタックアライメントは16バイト.
    pub const ALIGN: usize = 16;

    /// スタックのデフォルトレイアウト.
    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }

    /// 割り当てに成功した場合は、新しく割り当てられた
    /// プロセススタックをゼロ詰めにして返す。メモリが
    /// ない場合や他の理由でメモリの割り当てに失敗した
    /// 場合は `None` を返す.
    pub fn new() -> Option<Stack> {
        let raw_ptr = unsafe {
            let raw_ptr: *mut u8 = ALLOCATOR.alloc(Stack::layout());
            assert!(!raw_ptr.is_null());
            raw_ptr.write_bytes(0, Self::SIZE);
            raw_ptr
        };

        let ptr = Unique::new(raw_ptr as *mut _).expect("non-null");
        Some(Stack { ptr })
    }

    /// `*mut u8`にキャストするための内部メソッド.
    unsafe fn as_mut_ptr(&self) -> *mut u8 {
        self.ptr.as_ptr() as _
    }

    /// スタックの上端の物理アドレスを返す.
    pub fn top(&self) -> PhysicalAddr {
        unsafe { self.as_mut_ptr().add(Self::SIZE).into() }
    }

    /// スタックの底の物理アドレスを返すReturns the physical address of bottom of the stack.
    pub fn bottom(&self) -> PhysicalAddr {
        unsafe { self.as_mut_ptr().into() }
    }
}

impl Drop for Stack {
    fn drop(&mut self) {
        unsafe { ALLOCATOR.dealloc(self.as_mut_ptr(), Self::layout()) }
    }
}

impl fmt::Debug for Stack {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Stack")
            .field("top", &self.top())
            .field("bottom", &self.bottom())
            .field("size", &Self::SIZE)
            .finish()
    }
}
