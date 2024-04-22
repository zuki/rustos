#![allow(dead_code)]

use core::{fmt, ptr};

/// アドレスの _侵入型_ リンクリスト.
///
/// `LinkedList` は `*mut usize` のリストを保持する。`LinkedList` の
/// ユーザは渡されたポインタが有効で、ユニークで、少なくとも `usize`
/// サイズの書き込み可能なメモリを指していることを保証する。
///
/// # 使い方
///
/// リストは`LinkedList::new()`を使って作成する。新しいアドレスは
/// `push()` を使って先頭に追加することができる。リストの先頭のアドレスは、
/// もしあれば、 `pop()` を使って削除して返すか、 `peek()` を使って
/// （削除せずに）返すことができる。
///
/// ```rust
/// # let address_1 = (&mut (1 as usize)) as *mut usize;
/// # let address_2 = (&mut (2 as usize)) as *mut usize;
/// let mut list = LinkedList::new();
/// unsafe {
///     list.push(address_1);
///     list.push(address_2);
/// }
///
/// assert_eq!(list.peek(), Some(address_2));
/// assert_eq!(list.pop(), Some(address_2));
/// assert_eq!(list.pop(), Some(address_1));
/// assert_eq!(list.pop(), None);
/// ```
///
/// `LinkedList` は2つのイテレータを公開している。1つは `iter()` により
/// 得られるものでリスト内のすべてのアドレスをイテレートする。もう1つは
/// `iter_mut()` で返されるものでリスト内の各アドレスを参照する `Node` を
/// 返す。`Node` の `value()` メソッドと `pop()` メソッドを使って各々
/// リストから値を読み取ったり、 値をポップしたりすることができる。
///
/// ```rust
/// # let address_1 = (&mut (1 as usize)) as *mut usize;
/// # let address_2 = (&mut (2 as usize)) as *mut usize;
/// # let address_3 = (&mut (3 as usize)) as *mut usize;
/// let mut list = LinkedList::new();
/// unsafe {
///     list.push(address_1);
///     list.push(address_2);
///     list.push(address_3);
/// }
///
/// for node in list.iter_mut() {
///     if node.value() == address_2 {
///         node.pop();
///     }
/// }
///
/// assert_eq!(list.pop(), Some(address_3));
/// assert_eq!(list.pop(), Some(address_1));
/// assert_eq!(list.pop(), None);
/// ```
#[derive(Copy, Clone)]
pub struct LinkedList {
    head: *mut usize,
}

unsafe impl Send for LinkedList {}

impl LinkedList {
    /// 新しい空のインクリストを返す.
    pub const fn new() -> LinkedList {
        LinkedList {
            head: ptr::null_mut(),
        }
    }

    /// リストが空の場合は `true` 、それ以外は `false` を返す.
    pub fn is_empty(&self) -> bool {
        self.head.is_null()
    }

    /// アドレス `item` をリストの先頭にプッシュする.
    ///
    /// # 安全性
    ///
    /// 呼び出し元は `item` が `self` に存在する限り有効な、少なくとも
    /// `usize` サイズの一意で書き込み可能なメモリを参照していることを
    /// 保証しなければならない。一意性の制約を除けば、これはポインタが
    /// `self` に存在する限り `*item = some_usize` が安全な操作である
    /// ことを保証することと等価である。
    pub unsafe fn push(&mut self, item: *mut usize) {
        *item = self.head as usize;
        self.head = item;
    }

    /// もしあればリストの先頭のアイテムを削除して返す.
    pub fn pop(&mut self) -> Option<*mut usize> {
        let value = self.peek()?;
        self.head = unsafe { *value as *mut usize };
        Some(value)
    }

    /// もしあればリストの先頭のアイテムを削除せずに返す.
    pub fn peek(&self) -> Option<*mut usize> {
        match self.is_empty() {
            true => None,
            false => Some(self.head),
        }
    }

    /// このリストのアイテムを走査するイテレータを返す.
    pub fn iter(&self) -> Iter {
        Iter {
            current: self.head,
            _list: self,
        }
    }

    /// このリストのアイテムを走査するイテレータを返す.
    ///
    /// イテレータから返されるアイテム（`Node`型）は `Node::pop()`
    /// メソッドによってリンクリストから削除することができる。
    pub fn iter_mut(&mut self) -> IterMut {
        IterMut {
            prev: &mut self.head as *mut *mut usize as *mut usize,
            current: self.head,
            _list: self,
        }
    }
}

impl fmt::Debug for LinkedList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// リンクリストのアイテムを走査するイテレータ.
pub struct Iter<'a> {
    _list: &'a LinkedList,
    current: *mut usize,
}

impl<'a> Iterator for Iter<'a> {
    type Item = *mut usize;

    fn next(&mut self) -> Option<Self::Item> {
        let mut list = LinkedList { head: self.current };
        let value = list.pop()?;
        self.current = list.head;
        Some(value)
    }
}

/// `LinkedList` の可変イテレータから返されるアイテム.
pub struct Node {
    prev: *mut usize,
    value: *mut usize,
}

impl Node {
    /// このアイレムを所属するリンクリストから削除して返す.
    pub fn pop(self) -> *mut usize {
        unsafe {
            *(self.prev) = *(self.value);
        }
        self.value
    }

    /// この要素の値を返す.
    pub fn value(&self) -> *mut usize {
        self.value
    }
}

/// 可変を許すリンクリストのアイテムを走査するイテレータ.
pub struct IterMut<'a> {
    _list: &'a mut LinkedList,
    prev: *mut usize,
    current: *mut usize,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        let mut list = LinkedList { head: self.current };
        let value = list.pop()?;
        let prev = self.prev;
        self.prev = self.current;
        self.current = list.head;
        Some(Node { prev, value })
    }
}
