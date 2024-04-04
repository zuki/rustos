/// このクレートのラッパー型のすべてで実装されるトレイト.
///
/// ラッパーのinner型は関連するconstant `inner`により指定される。
/// これによりラッパー型の全てにおいてジェネリックな実装が可能になる。
pub trait Wrapper {
    /// ラップされる値の型.
    type Inner;

    /// ラップされているアイテムへのピンタを返す.
    fn ptr(&self) -> *const Self::Inner;
}

/// **readable** volatile ラッパーにより実装されるトレイト.
pub trait Readable<T> {
    /// innerポインタを返す.
    #[inline(always)]
    fn inner(&self) -> *const T;

    /// `self`が指している値を読んで返す. readは常にvolatileセマンティクスを
    /// 使って行われる.
    #[inline(always)]
    fn read(&self) -> T {
        unsafe { ::core::ptr::read_volatile(self.inner()) }
    }

    /// `self`が指している値がマスク値 `mask` を持っている場合、`true` を返す.
    /// これは `(self.read() & mask) == mask` に相当する.
    #[inline(always)]
    fn has_mask(&self, mask: T) -> bool
        where T: ::core::ops::BitAnd<Output = T>,
              T: PartialEq + Copy
    {
        (self.read() & mask) == mask
    }
}

/// **writeable** volatile ラッパーにより実装されるトレイト.
pub trait Writeable<T> {
    /// innerポインタを返す.
    #[inline(always)]
    fn inner(&mut self) -> *mut T;

    /// `self` のinnerアドレスに値 `val` を書き込む. writeは常に
    /// volatileセマンティクスを使って行われる.
    #[inline(always)]
    fn write(&mut self, val: T) {
        unsafe { ::core::ptr::write_volatile(self.inner(), val) }
    }
}

/// **readable _and_ writeable** volatile ラッパーにより実装されるトレイト.
pub trait ReadableWriteable<T>: Readable<T> + Writeable<T>
    where T: ::core::ops::BitAnd<Output = T>,
          T: ::core::ops::BitOr<Output = T>
{
    /// `self` が参照する値に `&` を使ってマスク値 `mask` を適用する.
    /// これは `self.write(self.read() & mask)` に相当する.
    fn and_mask(&mut self, mask: T) {
        let init_val = self.read();
        self.write(init_val & mask);
    }

    /// `self` が参照する値に `|` を使ってマスク値 `mask` を適用する.
    /// これは `self.write(self.read() | mask)` に相当する.
    fn or_mask(&mut self, mask: T) {
        let init_val = self.read();
        self.write(init_val | mask);
    }
}
