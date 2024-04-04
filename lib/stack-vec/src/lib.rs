#![no_std]

#[cfg(test)]
mod tests;

//use core::slice;
use core::iter::IntoIterator;
use core::ops::{Deref, DerefMut};
use core::slice::{Iter, IterMut};
//use core::option::IntoIter;

/// スライスを背後に持つ連続配列型
///
/// `StackVec`の機能は`std::Vec`に似ている。`push`と`pop`ができ、
/// ベクトル全体にわたってiterateすることができる。しかし、`Vec`とは
/// 異なり`StackVec`はメモリ割り当てが不要である。ユーザが指定した
/// スライスを愛護に持つからである。そのため、`StackVec`の容量は
/// ユーザが提供するスライスに _束縛_ される。この結果、`push` は
/// 失敗する可能性がある。`push`が呼ばれた際にベクタがfullの場合は
/// `Err`が返される。
#[derive(Debug)]
pub struct StackVec<'a, T: 'a> {
    storage: &'a mut [T],
    len: usize
}

impl<'a, T: 'a> StackVec<'a, T> {
    /// `storage`をバッキングストアとして新しく空の`StackVec<T>`構築する。
    /// 返される`StackVec`は`storage.len()`個の値を保持できる。
    pub fn new(storage: &'a mut [T]) -> StackVec<'a, T> {
        StackVec::with_len(storage, 0)
    }

    /// `storage`をバッキングストアとして新しい `StackVec<T>` を
    /// 作成する。storage`の最初の `len` 要素は `self.` に `push`
    /// されたものとして扱われる。戻り値の `StackVec` は合計
    /// `storage.len()` 個の値を保持できる。
    ///
    /// # Panics
    ///
    /// Panics if `len > storage.len()`.
    pub fn with_len(storage: &'a mut [T], len: usize) -> StackVec<'a, T> {
        if len > storage.len() {
            panic!("len > storage.len()");
        }

        StackVec { storage, len }
    }

    /// このベクタが保持できる要素の数を返す.
    pub fn capacity(&self) -> usize {
        self.storage.len()
    }

    /// 先頭から`len`個の要素を残してベクトルを短くする。
    /// `len`がベクタの現在の長さより大きい場合、このメソッドは
    /// 何もしない。このメソッドはベクトルの容量には影響しない
    /// ことに注意。
    pub fn truncate(&mut self, len: usize) {
        if len > self.len {
            return;
        }
        self.len = len;
    }

    /// `self`を消費してベクトル全体を含むスライスを取り出す。
    ///
    /// 返されるスライスの長さはこのベクトルの長さであり、元の
    /// ストレージの長さでは_ない_ことに注意
    pub fn into_slice(self) -> &'a mut [T] {
        &mut self.storage[0..self.len]
    }

    /// ベクタ全体を含むスライスを取り出す.
    pub fn as_slice(&self) -> &[T] {
        &self.storage[0..self.len]
    }

    /// ベクタ全体を含むミュータブルスライスを取り出す.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.storage[0..self.len]
    }

    /// ベクタの要素数（`length`とも呼ばれる）を返す.
    pub fn len(&self) -> usize {
        self.len
    }

    /// ベクタが要素を持たない場合、trueを返す.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// ベクタが容量に達している場合、trueを返す.
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    /// ベクタが満杯でなければ`value`をベクタの末尾に追加する.
    ///
    /// # Error
    ///
    /// このベクタが満杯の場合、`Err` が返される。創でなければ
    /// `Ok`が返される。
    pub fn push(&mut self, value: T) -> Result<(), ()> {
        if self.is_full() {
            return Err(());
        }

        self.storage[self.len] = value;
        self.len += 1;
        Ok(())
    }
}

impl<'a, T: Clone + 'a> StackVec<'a, T> {
    /// このベクタが空でない場合、このベクタから最後の要素を
    /// クローンして取り除き、それを返す。そうでない場合は
    /// `None` を返す。
    pub fn pop(&mut self) -> Option<T> {
        if self.is_empty() {
            return None;
        }
        self.len -= 1;
        Some(self.storage[self.len].clone())
    }
}

impl<'a, T: 'a> Deref for StackVec<'a, T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.storage[0..self.len]
    }
}

impl<'a, T: 'a> DerefMut for StackVec<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.storage
    }
}

impl<'a, T: 'a> IntoIterator for StackVec<'a, T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.into_slice().into_iter()
    }
}

impl<'a, T: 'a> IntoIterator for &'a StackVec<'a, T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.as_slice().iter()
    }
}
