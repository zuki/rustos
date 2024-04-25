use alloc::vec::Vec;
use core::mem::{align_of, forget, size_of};
use core::slice::{from_raw_parts, from_raw_parts_mut};

pub trait VecExt {
    /// `Vec<T>` を `Vec<U>` にキャストする.
    ///
    /// # 安全性
    ///
    /// callerは次の安全性プロパティを保証しなけらばならない:
    ///
    ///   * ベクタ `self` は型 `U` の有効な要素を含んでいる。特に、
    ///     `dorp` は `self` の `T` に対しては決して呼び出されず、
    ///     `self` の `U` 対して呼び出されることに注意すること。
    ///   * `T` と `U` のサイズとアライメントは同じである。
    ///
    /// # パニック
    ///
    /// `T` と `U` のサイズまたはアライメントが異なる場合はパニックになる。
    unsafe fn cast<U>(self) -> Vec<U>;
}

pub trait SliceExt {
    /// `&[T]` を `&[U]` にキャストする.
    ///
    /// # 安全性
    ///
    /// callerは次の安全性プロパティを保証しなけらばならない:
    ///
    ///   * スライス `self` は型 `U の有効な要素を含んでいる。
    ///   * `T` と `U` のサイズは同じである。
    ///   * `T` のアライメントは `U` のアライメントの整数倍である。
    ///
    /// # パニック
    ///
    /// `T` と `U` のサイズが異なる、または、`T` のアライメントが
    /// `U` のアライメントの整数倍でない場合はパニックにある。
    unsafe fn cast<'a, U>(&'a self) -> &'a [U];

    /// `&mut [T]` を `&mut [U]` にキャストする.
    ///
    /// # 安全性
    ///
    /// callerは次の安全性プロパティを保証しなけらばならない:
    ///
    ///   * スライス `self` は型 `U の有効な要素を含んでいる。
    ///   * `T` と `U` のサイズは同じである。
    ///   * `T` のアライメントは `U` のアライメントの整数倍である。
    ///
    /// # パニック
    ///
    /// `T` と `U` のサイズが異なる、または、`T` のアライメントが
    /// `U` のアライメントの整数倍でない場合はパニックにある。
    unsafe fn cast_mut<'a, U>(&'a mut self) -> &'a mut [U];
}

fn calc_new_len_cap<T, U>(vec: &Vec<T>) -> (usize, usize) {
    if size_of::<T>() > size_of::<U>() {
        assert!(size_of::<T>() % size_of::<U>() == 0);
        let factor = size_of::<T>() / size_of::<U>();
        (vec.len() * factor, vec.capacity() * factor)
    } else if size_of::<U>() > size_of::<T>() {
        assert!(size_of::<U>() % size_of::<T>() == 0);
        let factor = size_of::<U>() / size_of::<T>();
        (vec.len() / factor, vec.capacity() / factor)
    } else {
        (vec.len(), vec.capacity())
    }
}

impl<T> VecExt for Vec<T> {
    unsafe fn cast<U>(mut self) -> Vec<U> {
        assert!(align_of::<T>() == align_of::<U>());

        let (new_len, new_cap) = calc_new_len_cap::<T, U>(&self);
        let new_ptr = self.as_mut_ptr() as *mut U;
        forget(self);

        Vec::from_raw_parts(new_ptr, new_len, new_cap)
    }
}

fn calc_new_len<T, U>(slice: &[T]) -> usize {
    if size_of::<T>() > size_of::<U>() {
        assert!(size_of::<T>() % size_of::<U>() == 0);
        let factor = size_of::<T>() / size_of::<U>();
        slice.len() * factor
    } else if size_of::<U>() > size_of::<T>() {
        assert!(size_of::<U>() % size_of::<T>() == 0);
        let factor = size_of::<U>() / size_of::<T>();
        slice.len() / factor
    } else {
        slice.len()
    }
}

impl<T> SliceExt for [T] {
    unsafe fn cast<'a, U>(&'a self) -> &'a [U] {
        assert!(align_of::<T>() % align_of::<U>() == 0);

        let new_len = calc_new_len::<T, U>(self);
        let new_ptr = self.as_ptr() as *const U;
        from_raw_parts(new_ptr, new_len)
    }

    unsafe fn cast_mut<'a, U>(&'a mut self) -> &'a mut [U] {
        assert!(align_of::<T>() % align_of::<U>() == 0);

        let new_len = calc_new_len::<T, U>(self);
        let new_ptr = self.as_mut_ptr() as *mut U;
        from_raw_parts_mut(new_ptr, new_len)
    }
}
