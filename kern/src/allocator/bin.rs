use core::alloc::Layout;
use core::fmt::{Debug, Formatter, Result};
use core::ptr;

use crate::allocator::linked_list::LinkedList;
use crate::allocator::util::*;
use crate::allocator::LocalAlloc;

use crate::console::kprintln;

const NUM_BINS: usize = 30;

/// サイズクラスに基づいて割当を行うシンプルなアロケータ.
///   bin 0 (2^3 bytes)    : (0, 2^3]バイトの割り当てを処理する
///   bin 1 (2^4 bytes)    : (2^3, 2^4]バイトの割り当てを処理する
///   ...
///   bin 29 (2^32 bytes): (2^31, 2^32]バイトの割り当てを処理する
///
///   map_to_bin(size) -> k
///

pub struct Allocator {
    start: usize,
    end: usize,
    bins: [LinkedList; NUM_BINS]
}

/// `ptr`は`align`にアラインされているか.
fn has_alignment(ptr: usize, align: usize) -> bool {
    ptr % align == 0
}

impl Allocator {
    /// アドレス `start` から始まりアドレス `end` で終わる領域から
    /// メモリを割り当てる新しい bin アロケータを作成する.
    pub fn new(start: usize, end: usize) -> Allocator {
        Allocator {
            start: align_up(start, 8),          // 少なくとも8バイトアライン
            end,
            bins: [LinkedList::new(); NUM_BINS]
        }
    }

    /// ilog2(size)を返す
    fn ilog2(&self, size: usize) -> usize {
        (63 - size.leading_zeros()) as usize
    }

    /// サイズからbins配列のインデックスを取得する
    fn map_to_bin(&self, size: usize) -> usize {
        if size < 7 {
            0
        } else if size.count_ones() == 1 {
            self.ilog2(size) - 3
        } else {
            self.ilog2(size) - 2
        }
    }

    /// binのビンサイズを返す
    fn bin_size(&self, bin: usize) -> usize {
        1_usize << (bin + 3)
    }

    /// binのメモリを1つ取り出して2つに分割して、(bin-1)にpushする.
    /// 分割できたらtrue, そうでなければfalseを返す
    fn split_bin(&mut self, bin: usize) -> bool {
        if bin == 0 {
            return false; // 最小サイズのbinは分割できない
        }

        match self.bins[bin].pop() {
            None => false,          // このbinには提供できるメモリはない
            Some(ptr) => {          // binのエントリを2つに分割して(bin-1)のエントリとして使用
                let sub_size = self.bin_size(bin - 1);

                unsafe {
                    self.bins[bin - 1].push(ptr);
                    self.bins[bin - 1].push(((ptr as usize) + sub_size) as *mut usize);
                }

                true
            }
        }
    }

    /// 大きなアライメントのために無駄になるスペースをできる限り小さなビンで使用する。
    /// 空き巣ベースがすべてビンエントリとして使用できた場合は true,
    /// 利用できなかったスベースが残った場合は false
    fn fill_allocations_until(&mut self, end: usize) -> bool {
        'fill_loop: while self.start != end {
            assert!(end - self.start > 7);

            // 大きなビンから順に試す
            for i in (0..self.bins.len()).rev() {
                // 1. ビンサイズを取得して
                let bin_size = self.bin_size(i);
                // 2. このビンのビンサイズはこの領域で利用可能、先頭がビンサイズでアライン
                if self.start + bin_size < end && has_alignment(self.start, bin_size) {
                    // 3. このビンのエントリとして登録
                    self.allocate_bin_entry(i);
                    // 4. startが更新されているのでfill_loopを継続
                    continue 'fill_loop;
                }
            }
            // 空きスペースは使用できない部分があった
            return false;
        }
        // 空きスペースはすべて使用できた
        true
    }

    /// binにエントリを追加する
    fn allocate_bin_entry(&mut self, bin: usize) -> bool {
        loop {
            // 1. startをビンサイズでアライメント
            let alloc_start = align_up(self.start, self.bin_size(bin));
            // 2. ビンサイズを取得
            let bin_size = self.bin_size(bin);

            // 3. 割り当てるメモリがない
            if alloc_start + bin_size > self.end {
                return false;
            }

            // 4. アライメントで空いたスペースをbinより小さなbinのエントリで埋める
            self.fill_allocations_until(alloc_start);

            // 5. 割り当て可能開始位置を更新する
            self.start = alloc_start + bin_size;

            // 6. binエントリとして登録する
            unsafe { self.bins[bin].push(alloc_start as *mut usize) };

            return true;
        }
    }

    /// layoutのサイズとアライメントの大きい方でbinを決める
    fn layout_to_bin(&self, layout: Layout) -> usize {
        self.map_to_bin(core::cmp::max(layout.size(), layout.align()))
    }

    /// 上位binのエントリを取得して分割してbinのエントリを作成する
    fn recursive_split_bin(&mut self, bin: usize) -> bool {
        // 1. 対象のbinにはエントリがある場合は何もしない
        if !self.bins[bin].is_empty() {
            return true;
        }

        // 2. 対象のbinが最大binであるので上位binはなく分割取得できない
        if bin == self.bins.len() - 1 {
            return false;
        }

        // 3. 上位のbinからの分割取得に失敗した
        if self.bins[bin + 1].is_empty() && !self.recursive_split_bin(bin + 1) {
            return false;
        }

        // 4. 1つ上のbinの分割に失敗した
        if !self.split_bin(bin + 1) {
            return false;
        }

        // 5. 分割が成功した場合はエントリが登録されている
        !self.bins[bin].is_empty()
    }

    /// binにエントリを追加する
    fn scavenge_bin(&mut self, bin: usize) -> bool {
        // 1. 対象のbinより上位のbinのエントリを取り出して分割して登録する。
        if self.recursive_split_bin(bin) {
            return true;
        }

        // 2. メモリ領域から割り当てをする
        self.allocate_bin_entry(bin)
    }

    fn do_alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        // 1. 使用するbinを決定する
        let bin = self.layout_to_bin(layout);

        // 2. binにエントリがあればそれを返す。
        if let Some(p) = self.bins[bin].pop() {
            return Some(p as *mut u8);
        }

        // 3. binのエントリをメモリ領域から作成する
        if !self.scavenge_bin(bin) {
            return None;
        }

        Some(self.bins[bin].pop().unwrap() as *mut u8)
    }

    fn do_dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let bin = self.layout_to_bin(layout);
        // 8バイトにアラインされたポインタしか渡さないのでキャストは安全
        unsafe { self.bins[bin].push(ptr as *mut usize) };
    }

}

impl LocalAlloc for Allocator {
    /// メモリを割り当てる。`layout.size()`と`layout.align()`の
    /// サイズプロパティとアライメントプロパティを満たすポインタを返す。
    ///
    /// このメソッドが`Ok(addr)`を返す場合、`addr`は`layout`の
    /// インスタンスを保持するのに適したストレージのブロックを
    /// 指す非NULのアドレスとなる。特に、このブロックは少なくとも
    /// `layout.size()`バイトの大きさであり、`layout.align()`に
    /// アラインメントされている。返されたストレージのブロックは
    /// その内容が初期化またはゼロ詰めになっていても、いなくてもよい。
    ///
    /// # 安全性
    ///
    /// _caller_ は`layout.size() > 0`と`layout.align()`が2のべき乗で
    /// あることを保証しなければならない。これらの条件を満たさない
    /// パラメータは未定義動作を引き起こす可能性がある。
    ///
    /// # エラー
    ///
    /// ヌルポインタ (`core::ptr::null_mut`) が返された場合はメモリを
    /// 使い果たしたか、 `layout`がこのアロケータのサイズまたは
    /// アラインメントの制約を満たしていないことを示す。
    unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        self.do_alloc(layout).unwrap_or(0 as *mut u8)
    }

    /// `ptr`で参照されるメモリの割当を解除する.
    ///
    /// # 安全性
    ///
    /// _caller_は以下を保証しなければならない:
    ///
    ///   * `ptr`はアロケータ経由で現在割り当てられているメモリブロックを
    ///     示していなければならない。
    ///   * `layout`は`ptr`を返した割り当てコールに使用した元の
    ///     レイアウトを正しく表していなければならない。
    ///
    /// これらの制約を満たしていないパラメタは未定義動作をする
    /// 可能性がある。
    unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        self.do_dealloc(ptr, layout);
    }
}

// FIXME: Implement `Debug` for `Allocator`.
impl Debug for Allocator {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Allocator")
         .field("Start", &self.start)
         .field("end", &self.end)
         .finish()
    }
}
