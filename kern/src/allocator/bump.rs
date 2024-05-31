use core::alloc::Layout;
//use core::ptr;

use crate::allocator::util::*;
use crate::allocator::LocalAlloc;

/// "バンプ"アロケータ: ポインタを動かすことでメモリを割り当てる。
/// 開放はしない。
#[derive(Debug)]
pub struct Allocator {
    current: usize,
    end: usize,
}

impl Allocator {
    /// アドレス `start` から始まりアドレス `end` で終わる領域から
    /// メモリを割り当てる新しいバンプアロケータを作成する。
    #[allow(dead_code)]
    pub fn new(start: usize, end: usize) -> Allocator {
        Allocator {
            current: start,
            end
        }
    }
}

impl LocalAlloc for Allocator {
    /// メモリを割り当てる。`layout.size()`と`layout.align()`の
    /// プロパティを満たすポインタを返す。
    ///
    /// このメソッドが`Ok(addr)`を返す場合、`addr`は`layout`の
    /// インスタンスを保持するのに適したストレージのブロックを
    /// 指す非NULのアドレスとなる。特に、このブロックは少なくとも
    /// `layout.size()`バイトの大きさであり、`layout.align()`に
    /// アラインメントされている。返されたストレージのブロックは
    /// その内容が初期化またはゼロ詰めになっていても、いなくてもよい。
    ///
    /// # Safety
    ///
    /// _caller_ は`layout.size() > 0`と`layout.align()`が2のべき乗で
    /// あることを保証しなければならない。これらの条件を満たさない
    /// パラメータは未定義の動作を引き起こす可能性がある。
    ///
    /// # Errors
    ///
    /// ヌルポインタ (`core::ptr::null_mut`) が返された場合はメモリを
    /// 使い果たしたか、 `layout`がこのアロケータのサイズまたは
    /// アラインメントの制約を満たしていないことを示す。
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 || layout.align().count_ones() != 1 {
            return core::ptr::null_mut() as *mut u8;
        }

        let addr_start = align_up(self.current, layout.align());
        let addr_end = addr_start.saturating_add(layout.size());
        if self.end <= addr_start || self.end < addr_end {
            return core::ptr::null_mut() as *mut u8;
        }

        self.current = addr_end;
        addr_start as *mut u8
    }

    /// `ptr`で参照されるメモリの割当を解除する.
    ///
    /// # Safety
    ///
    /// _caller_は以下を保証しなければならない:
    ///
    ///   * `ptr`はアロケータ経由で現在割り当てられているメモリブロックを
    ///     示していなければならない。
    ///   * `layout`は`ptr`を返した割り当てコールに使用した元の
    ///     レイアウトを正しく表していなければならない。
    ///
    /// これらの制約を満たしていないパラメタは未定義の結果になる
    /// 可能性がある。
    unsafe fn dealloc(&mut self, _ptr: *mut u8, _layout: Layout) {
        // LEAKED
    }
}
