// ignore-tidy-filelength

//! クロスプラットフォームのパス操作.
//!
//! このモジュールは抽象的にパスを操作するための2つの型 [`PathBuf`] と
//! [`Path`][`Path`] （[`String`] と [`str`] のようなもの）を提供する。
//! これらの型はそれぞれ [`OsString`] と [`OsStr`] の薄いラッパーであり、
//! ローカルプラットフォームのパス構文に従う文字列を直接扱うことができる。
//!
//! パスは [`Path`] の [`components`] メソッドが返す構造体を反復処理する
//! ことで [`Component`] にパースすることができる。[`Component`] は大まかに
//! 言えば、パスの区切り文字 (`/` または `\`) の間の部分文字列に対応する。
//! コンポーネントから相当するパスを再構築するには [`PathBuf`] に対して
//! [`push`] メソッドを使用する。[`components`] メソッドのドキュメントで
//! 説明されている正規化によってパスの構文が変わる可能性があることに注意
//! されたい。
//!
//! ## 簡単な使用例
//!
//! Pathの操作にはスライスからコンポーネントをパースする操作と新規にpathを
//! 構築する操作の2つがある。
//!
//! pathのパースには [`str`] スライスから [`Path`] を作成して、質問を
//! 開始することができる。
//!
//! ```
//! use std::path::Path;
//! use std::ffi::OsStr;
//!
//! let path = Path::new("/tmp/foo/bar.txt");
//!
//! let parent = path.parent();
//! assert_eq!(parent, Some(Path::new("/tmp/foo")));
//!
//! let file_stem = path.file_stem();
//! assert_eq!(file_stem, Some(OsStr::new("bar")));
//!
//! let extension = path.extension();
//! assert_eq!(extension, Some(OsStr::new("txt")));
//! ```
//!
//! pathの構築と変更には [`PathBuf`] を使用する。
//!
//! ```
//! use std::path::PathBuf;
//!
//! // これで動作するはず...
//! let mut path = PathBuf::from("c:\\");
//!
//! path.push("windows");
//! path.push("system32");
//!
//! path.set_extension("dll");
//!
//! // ... しかし、Pushは事前にすべてを把握していない場合に
//! // 使うのがベストであり、把握しているのであれば次の方法のが
//! // 良い。
//! let path: PathBuf = ["c:\\", "windows", "system32.dll"].iter().collect();
//! ```
//!
//! [`Component`]: ../../std/path/enum.Component.html
//! [`components`]: ../../std/path/struct.Path.html#method.components
//! [`PathBuf`]: ../../std/path/struct.PathBuf.html
//! [`Path`]: ../../std/path/struct.Path.html
//! [`push`]: ../../std/path/struct.PathBuf.html#method.push
//! [`String`]: ../../std/string/struct.String.html
//!
//! [`str`]: ../../std/primitive.str.html
//! [`OsString`]: ../../std/ffi/struct.OsString.html
//! [`OsStr`]: ../../std/ffi/struct.OsStr.html

use alloc::borrow::{Borrow, Cow, ToOwned};
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::str::FromStr;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp;
use core::fmt;
use core::hash::{Hash, Hasher};
use core::iter::{self, FusedIterator};
use core::ops::{self, Deref};

use crate::ffi::{OsStr, OsString};

#[inline]
pub fn is_sep_byte(b: u8) -> bool {
    b == b'/'
}

#[inline]
pub fn is_verbatim_sep(b: u8) -> bool {
    b == b'/'
}

pub fn parse_prefix(_: &OsStr) -> Option<Prefix<'_>> {
    None
}

pub const MAIN_SEP_STR: &str = "/";
pub const MAIN_SEP: char = '/';

////////////////////////////////////////////////////////////////////////////////
// GENERAL NOTES
////////////////////////////////////////////////////////////////////////////////
//
// このモジュールでの解析はOsStrが常にASCII文字をそのままエンコードする
// ことを利用して OsStr を [u8] スライスに直接変換することで行われる。
// 最終的には、この変換は OsStr のAPIを直接使ってパースすることに取って
// 代わられるはずであるが、それが可能になるまでにはしばらく時間がかかるだろう。

////////////////////////////////////////////////////////////////////////////////
// Windows Prefixes
////////////////////////////////////////////////////////////////////////////////

/// Windows path prefixes, e.g., `C:` or `\\server\share`.
///
/// Windows uses a variety of path prefix styles, including references to drive
/// volumes (like `C:`), network shared folders (like `\\server\share`), and
/// others. In addition, some path prefixes are "verbatim" (i.e., prefixed with
/// `\\?\`), in which case `/` is *not* treated as a separator and essentially
/// no normalization is performed.
///
/// # Examples
///
/// ```
/// use std::path::{Component, Path, Prefix};
/// use std::path::Prefix::*;
/// use std::ffi::OsStr;
///
/// fn get_path_prefix(s: &str) -> Prefix {
///     let path = Path::new(s);
///     match path.components().next().unwrap() {
///         Component::Prefix(prefix_component) => prefix_component.kind(),
///         _ => panic!(),
///     }
/// }
///
/// # if cfg!(windows) {
/// assert_eq!(Verbatim(OsStr::new("pictures")),
///            get_path_prefix(r"\\?\pictures\kittens"));
/// assert_eq!(VerbatimUNC(OsStr::new("server"), OsStr::new("share")),
///            get_path_prefix(r"\\?\UNC\server\share"));
/// assert_eq!(VerbatimDisk(b'C'), get_path_prefix(r"\\?\c:\"));
/// assert_eq!(DeviceNS(OsStr::new("BrainInterface")),
///            get_path_prefix(r"\\.\BrainInterface"));
/// assert_eq!(UNC(OsStr::new("server"), OsStr::new("share")),
///            get_path_prefix(r"\\server\share"));
/// assert_eq!(Disk(b'C'), get_path_prefix(r"C:\Users\Rust\Pictures\Ferris"));
/// # }
/// ```
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub enum Prefix<'a> {
    /// Verbatim prefix, e.g., `\\?\cat_pics`.
    ///
    /// Verbatim prefixes consist of `\\?\` immediately followed by the given
    /// component.
    Verbatim(&'a OsStr),

    /// Verbatim prefix using Windows' _**U**niform **N**aming **C**onvention_,
    /// e.g., `\\?\UNC\server\share`.
    ///
    /// Verbatim UNC prefixes consist of `\\?\UNC\` immediately followed by the
    /// server's hostname and a share name.
    VerbatimUNC(&'a OsStr, &'a OsStr),

    /// Verbatim disk prefix, e.g., `\\?\C:\`.
    ///
    /// Verbatim disk prefixes consist of `\\?\` immediately followed by the
    /// drive letter and `:\`.
    VerbatimDisk(u8),

    /// Device namespace prefix, e.g., `\\.\COM42`.
    ///
    /// Device namespace prefixes consist of `\\.\` immediately followed by the
    /// device name.
    DeviceNS(&'a OsStr),

    /// Prefix using Windows' _**U**niform **N**aming **C**onvention_, e.g.
    /// `\\server\share`.
    ///
    /// UNC prefixes consist of the server's hostname and a share name.
    UNC(&'a OsStr, &'a OsStr),

    /// Prefix `C:` for the given disk drive.
    Disk(u8),
}

impl<'a> Prefix<'a> {
    #[inline]
    fn len(&self) -> usize {
        use self::Prefix::*;
        fn os_str_len(s: &OsStr) -> usize {
            os_str_as_u8_slice(s).len()
        }
        match *self {
            Verbatim(x) => 4 + os_str_len(x),
            VerbatimUNC(x, y) => {
                8 + os_str_len(x)
                    + if os_str_len(y) > 0 {
                        1 + os_str_len(y)
                    } else {
                        0
                    }
            }
            VerbatimDisk(_) => 6,
            UNC(x, y) => {
                2 + os_str_len(x)
                    + if os_str_len(y) > 0 {
                        1 + os_str_len(y)
                    } else {
                        0
                    }
            }
            DeviceNS(x) => 4 + os_str_len(x),
            Disk(_) => 2,
        }
    }

    /// Determines if the prefix is verbatim, i.e., begins with `\\?\`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Prefix::*;
    /// use std::ffi::OsStr;
    ///
    /// assert!(Verbatim(OsStr::new("pictures")).is_verbatim());
    /// assert!(VerbatimUNC(OsStr::new("server"), OsStr::new("share")).is_verbatim());
    /// assert!(VerbatimDisk(b'C').is_verbatim());
    /// assert!(!DeviceNS(OsStr::new("BrainInterface")).is_verbatim());
    /// assert!(!UNC(OsStr::new("server"), OsStr::new("share")).is_verbatim());
    /// assert!(!Disk(b'C').is_verbatim());
    /// ```
    #[inline]
    pub fn is_verbatim(&self) -> bool {
        use self::Prefix::*;
        match *self {
            Verbatim(_) | VerbatimDisk(_) | VerbatimUNC(..) => true,
            _ => false,
        }
    }

    #[inline]
    fn is_drive(&self) -> bool {
        match *self {
            Prefix::Disk(_) => true,
            _ => false,
        }
    }

    #[inline]
    fn has_implicit_root(&self) -> bool {
        !self.is_drive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Exposed parsing helpers
////////////////////////////////////////////////////////////////////////////////

/// Determines whether the character is one of the permitted path
/// separators for the current platform.
///
/// # Examples
///
/// ```
/// use std::path;
///
/// assert!(path::is_separator('/')); // '/' works for both Unix and Windows
/// assert!(!path::is_separator('❤'));
/// ```
pub fn is_separator(c: char) -> bool {
    c.is_ascii() && is_sep_byte(c as u8)
}

/// The primary separator of path components for the current platform.
///
/// For example, `/` on Unix and `\` on Windows.
pub const MAIN_SEPARATOR: char = '/';

////////////////////////////////////////////////////////////////////////////////
// Misc helpers
////////////////////////////////////////////////////////////////////////////////

// Iterate through `iter` while it matches `prefix`; return `None` if `prefix`
// is not a prefix of `iter`, otherwise return `Some(iter_after_prefix)` giving
// `iter` after having exhausted `prefix`.
fn iter_after<'a, 'b, I, J>(mut iter: I, mut prefix: J) -> Option<I>
where
    I: Iterator<Item = Component<'a>> + Clone,
    J: Iterator<Item = Component<'b>>,
{
    loop {
        let mut iter_next = iter.clone();
        match (iter_next.next(), prefix.next()) {
            (Some(ref x), Some(ref y)) if x == y => (),
            (Some(_), Some(_)) => return None,
            (Some(_), None) => return Some(iter),
            (None, None) => return Some(iter),
            (None, Some(_)) => return None,
        }
        iter = iter_next;
    }
}

// See note at the top of this module to understand why these are used:
fn os_str_as_u8_slice(s: &OsStr) -> &[u8] {
    unsafe { &*(s as *const OsStr as *const [u8]) }
}
unsafe fn u8_slice_as_os_str(s: &[u8]) -> &OsStr {
    &*(s as *const [u8] as *const OsStr)
}

// Detect scheme on Redox
fn has_redox_scheme(s: &[u8]) -> bool {
    cfg!(target_os = "redox") && s.contains(&b':')
}

////////////////////////////////////////////////////////////////////////////////
// Cross-platform, iterator-independent parsing
////////////////////////////////////////////////////////////////////////////////

/// Says whether the first byte after the prefix is a separator.
fn has_physical_root(s: &[u8], prefix: Option<Prefix<'_>>) -> bool {
    let path = if let Some(p) = prefix {
        &s[p.len()..]
    } else {
        s
    };
    !path.is_empty() && is_sep_byte(path[0])
}

// basic workhorse for splitting stem and extension
fn split_file_at_dot(file: &OsStr) -> (Option<&OsStr>, Option<&OsStr>) {
    unsafe {
        if os_str_as_u8_slice(file) == b".." {
            return (Some(file), None);
        }

        // The unsafety here stems from converting between &OsStr and &[u8]
        // and back. This is safe to do because (1) we only look at ASCII
        // contents of the encoding and (2) new &OsStr values are produced
        // only from ASCII-bounded slices of existing &OsStr values.

        let mut iter = os_str_as_u8_slice(file).rsplitn(2, |b| *b == b'.');
        let after = iter.next();
        let before = iter.next();
        if before == Some(b"") {
            (Some(file), None)
        } else {
            (
                before.map(|s| u8_slice_as_os_str(s)),
                after.map(|s| u8_slice_as_os_str(s)),
            )
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// The core iterators
////////////////////////////////////////////////////////////////////////////////

/// Component parsing works by a double-ended state machine; the cursors at the
/// front and back of the path each keep track of what parts of the path have
/// been consumed so far.
///
/// Going front to back, a path is made up of a prefix, a starting
/// directory component, and a body (of normal components)
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
enum State {
    Prefix = 0,   // c:
    StartDir = 1, // / or . or nothing
    Body = 2,     // foo/bar/baz
    Done = 3,
}

/// A structure wrapping a Windows path prefix as well as its unparsed string
/// representation.
///
/// In addition to the parsed [`Prefix`] information returned by [`kind`],
/// `PrefixComponent` also holds the raw and unparsed [`OsStr`] slice,
/// returned by [`as_os_str`].
///
/// Instances of this `struct` can be obtained by matching against the
/// [`Prefix` variant] on [`Component`].
///
/// Does not occur on Unix.
///
/// # Examples
///
/// ```
/// # if cfg!(windows) {
/// use std::path::{Component, Path, Prefix};
/// use std::ffi::OsStr;
///
/// let path = Path::new(r"c:\you\later\");
/// match path.components().next().unwrap() {
///     Component::Prefix(prefix_component) => {
///         assert_eq!(Prefix::Disk(b'C'), prefix_component.kind());
///         assert_eq!(OsStr::new("c:"), prefix_component.as_os_str());
///     }
///     _ => unreachable!(),
/// }
/// # }
/// ```
///
/// [`as_os_str`]: #method.as_os_str
/// [`Component`]: enum.Component.html
/// [`kind`]: #method.kind
/// [`OsStr`]: ../../std/ffi/struct.OsStr.html
/// [`Prefix` variant]: enum.Component.html#variant.Prefix
/// [`Prefix`]: enum.Prefix.html
#[derive(Copy, Clone, Eq, Debug)]
pub struct PrefixComponent<'a> {
    /// The prefix as an unparsed `OsStr` slice.
    raw: &'a OsStr,

    /// The parsed prefix data.
    parsed: Prefix<'a>,
}

impl<'a> PrefixComponent<'a> {
    /// Returns the parsed prefix data.
    ///
    /// See [`Prefix`]'s documentation for more information on the different
    /// kinds of prefixes.
    ///
    /// [`Prefix`]: enum.Prefix.html
    pub fn kind(&self) -> Prefix<'a> {
        self.parsed
    }

    /// Returns the raw [`OsStr`] slice for this prefix.
    ///
    /// [`OsStr`]: ../../std/ffi/struct.OsStr.html
    pub fn as_os_str(&self) -> &'a OsStr {
        self.raw
    }
}

impl<'a> cmp::PartialEq for PrefixComponent<'a> {
    fn eq(&self, other: &PrefixComponent<'a>) -> bool {
        cmp::PartialEq::eq(&self.parsed, &other.parsed)
    }
}

impl<'a> cmp::PartialOrd for PrefixComponent<'a> {
    fn partial_cmp(&self, other: &PrefixComponent<'a>) -> Option<cmp::Ordering> {
        cmp::PartialOrd::partial_cmp(&self.parsed, &other.parsed)
    }
}

impl cmp::Ord for PrefixComponent<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        cmp::Ord::cmp(&self.parsed, &other.parsed)
    }
}

impl Hash for PrefixComponent<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.parsed.hash(h);
    }
}

/// A single component of a path.
///
/// A `Component` roughly corresponds to a substring between path separators
/// (`/` or `\`).
///
/// This `enum` is created by iterating over [`Components`], which in turn is
/// created by the [`components`][`Path::components`] method on [`Path`].
///
/// # Examples
///
/// ```rust
/// use std::path::{Component, Path};
///
/// let path = Path::new("/tmp/foo/bar.txt");
/// let components = path.components().collect::<Vec<_>>();
/// assert_eq!(&components, &[
///     Component::RootDir,
///     Component::Normal("tmp".as_ref()),
///     Component::Normal("foo".as_ref()),
///     Component::Normal("bar.txt".as_ref()),
/// ]);
/// ```
///
/// [`Components`]: struct.Components.html
/// [`Path`]: struct.Path.html
/// [`Path::components`]: struct.Path.html#method.components
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Component<'a> {
    /// A Windows path prefix, e.g., `C:` or `\\server\share`.
    ///
    /// There is a large variety of prefix types, see [`Prefix`]'s documentation
    /// for more.
    ///
    /// Does not occur on Unix.
    ///
    /// [`Prefix`]: enum.Prefix.html
    Prefix(PrefixComponent<'a>),

    /// The root directory component, appears after any prefix and before anything else.
    ///
    /// It represents a separator that designates that a path starts from root.
    RootDir,

    /// A reference to the current directory, i.e., `.`.
    CurDir,

    /// A reference to the parent directory, i.e., `..`.
    ParentDir,

    /// A normal component, e.g., `a` and `b` in `a/b`.
    ///
    /// This variant is the most common one, it represents references to files
    /// or directories.
    Normal(&'a OsStr),
}

impl<'a> Component<'a> {
    /// Extracts the underlying [`OsStr`] slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("./tmp/foo/bar.txt");
    /// let components: Vec<_> = path.components().map(|comp| comp.as_os_str()).collect();
    /// assert_eq!(&components, &[".", "tmp", "foo", "bar.txt"]);
    /// ```
    ///
    /// [`OsStr`]: ../../std/ffi/struct.OsStr.html
    pub fn as_os_str(self) -> &'a OsStr {
        match self {
            Component::Prefix(p) => p.as_os_str(),
            Component::RootDir => OsStr::new(MAIN_SEP_STR),
            Component::CurDir => OsStr::new("."),
            Component::ParentDir => OsStr::new(".."),
            Component::Normal(path) => path,
        }
    }
}

impl AsRef<OsStr> for Component<'_> {
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

impl AsRef<Path> for Component<'_> {
    fn as_ref(&self) -> &Path {
        self.as_os_str().as_ref()
    }
}

/// An iterator over the [`Component`]s of a [`Path`].
///
/// This `struct` is created by the [`components`] method on [`Path`].
/// See its documentation for more.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("/tmp/foo/bar.txt");
///
/// for component in path.components() {
///     println!("{:?}", component);
/// }
/// ```
///
/// [`Component`]: enum.Component.html
/// [`components`]: struct.Path.html#method.components
/// [`Path`]: struct.Path.html
#[derive(Clone)]
pub struct Components<'a> {
    // The path left to parse components from
    path: &'a [u8],

    // The prefix as it was originally parsed, if any
    prefix: Option<Prefix<'a>>,

    // true if path *physically* has a root separator; for most Windows
    // prefixes, it may have a "logical" rootseparator for the purposes of
    // normalization, e.g.,  \\server\share == \\server\share\.
    has_physical_root: bool,

    // The iterator is double-ended, and these two states keep track of what has
    // been produced from either end
    front: State,
    back: State,
}

/// An iterator over the [`Component`]s of a [`Path`], as [`OsStr`] slices.
///
/// This `struct` is created by the [`iter`] method on [`Path`].
/// See its documentation for more.
///
/// [`Component`]: enum.Component.html
/// [`iter`]: struct.Path.html#method.iter
/// [`OsStr`]: ../../std/ffi/struct.OsStr.html
/// [`Path`]: struct.Path.html
#[derive(Clone)]
pub struct Iter<'a> {
    inner: Components<'a>,
}

impl fmt::Debug for Components<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct DebugHelper<'a>(&'a Path);

        impl fmt::Debug for DebugHelper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.components()).finish()
            }
        }

        f.debug_tuple("Components")
            .field(&DebugHelper(self.as_path()))
            .finish()
    }
}

impl<'a> Components<'a> {
    // how long is the prefix, if any?
    #[inline]
    fn prefix_len(&self) -> usize {
        self.prefix.as_ref().map(Prefix::len).unwrap_or(0)
    }

    #[inline]
    fn prefix_verbatim(&self) -> bool {
        self.prefix
            .as_ref()
            .map(Prefix::is_verbatim)
            .unwrap_or(false)
    }

    /// how much of the prefix is left from the point of view of iteration?
    #[inline]
    fn prefix_remaining(&self) -> usize {
        if self.front == State::Prefix {
            self.prefix_len()
        } else {
            0
        }
    }

    // Given the iteration so far, how much of the pre-State::Body path is left?
    #[inline]
    fn len_before_body(&self) -> usize {
        let root = if self.front <= State::StartDir && self.has_physical_root {
            1
        } else {
            0
        };
        let cur_dir = if self.front <= State::StartDir && self.include_cur_dir() {
            1
        } else {
            0
        };
        self.prefix_remaining() + root + cur_dir
    }

    // is the iteration complete?
    #[inline]
    fn finished(&self) -> bool {
        self.front == State::Done || self.back == State::Done || self.front > self.back
    }

    #[inline]
    fn is_sep_byte(&self, b: u8) -> bool {
        if self.prefix_verbatim() {
            is_verbatim_sep(b)
        } else {
            is_sep_byte(b)
        }
    }

    /// Extracts a slice corresponding to the portion of the path remaining for iteration.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut components = Path::new("/tmp/foo/bar.txt").components();
    /// components.next();
    /// components.next();
    ///
    /// assert_eq!(Path::new("foo/bar.txt"), components.as_path());
    /// ```
    pub fn as_path(&self) -> &'a Path {
        let mut comps = self.clone();
        if comps.front == State::Body {
            comps.trim_left();
        }
        if comps.back == State::Body {
            comps.trim_right();
        }
        unsafe { Path::from_u8_slice(comps.path) }
    }

    /// Is the *original* path rooted?
    fn has_root(&self) -> bool {
        if self.has_physical_root {
            return true;
        }
        if let Some(p) = self.prefix {
            if p.has_implicit_root() {
                return true;
            }
        }
        false
    }

    /// Should the normalized path include a leading . ?
    fn include_cur_dir(&self) -> bool {
        if self.has_root() {
            return false;
        }
        let mut iter = self.path[self.prefix_len()..].iter();
        match (iter.next(), iter.next()) {
            (Some(&b'.'), None) => true,
            (Some(&b'.'), Some(&b)) => self.is_sep_byte(b),
            _ => false,
        }
    }

    // parse a given byte sequence into the corresponding path component
    fn parse_single_component<'b>(&self, comp: &'b [u8]) -> Option<Component<'b>> {
        match comp {
            b"." if self.prefix_verbatim() => Some(Component::CurDir),
            b"." => None, // . components are normalized away, except at
            // the beginning of a path, which is treated
            // separately via `include_cur_dir`
            b".." => Some(Component::ParentDir),
            b"" => None,
            _ => Some(Component::Normal(unsafe { u8_slice_as_os_str(comp) })),
        }
    }

    // parse a component from the left, saying how many bytes to consume to
    // remove the component
    fn parse_next_component(&self) -> (usize, Option<Component<'a>>) {
        debug_assert!(self.front == State::Body);
        let (extra, comp) = match self.path.iter().position(|b| self.is_sep_byte(*b)) {
            None => (0, self.path),
            Some(i) => (1, &self.path[..i]),
        };
        (comp.len() + extra, self.parse_single_component(comp))
    }

    // parse a component from the right, saying how many bytes to consume to
    // remove the component
    fn parse_next_component_back(&self) -> (usize, Option<Component<'a>>) {
        debug_assert!(self.back == State::Body);
        let start = self.len_before_body();
        let (extra, comp) = match self.path[start..]
            .iter()
            .rposition(|b| self.is_sep_byte(*b))
        {
            None => (0, &self.path[start..]),
            Some(i) => (1, &self.path[start + i + 1..]),
        };
        (comp.len() + extra, self.parse_single_component(comp))
    }

    // trim away repeated separators (i.e., empty components) on the left
    fn trim_left(&mut self) {
        while !self.path.is_empty() {
            let (size, comp) = self.parse_next_component();
            if comp.is_some() {
                return;
            } else {
                self.path = &self.path[size..];
            }
        }
    }

    // trim away repeated separators (i.e., empty components) on the right
    fn trim_right(&mut self) {
        while self.path.len() > self.len_before_body() {
            let (size, comp) = self.parse_next_component_back();
            if comp.is_some() {
                return;
            } else {
                self.path = &self.path[..self.path.len() - size];
            }
        }
    }
}

impl AsRef<Path> for Components<'_> {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for Components<'_> {
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}

impl fmt::Debug for Iter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct DebugHelper<'a>(&'a Path);

        impl fmt::Debug for DebugHelper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.iter()).finish()
            }
        }

        f.debug_tuple("Iter")
            .field(&DebugHelper(self.as_path()))
            .finish()
    }
}

impl<'a> Iter<'a> {
    /// Extracts a slice corresponding to the portion of the path remaining for iteration.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut iter = Path::new("/tmp/foo/bar.txt").iter();
    /// iter.next();
    /// iter.next();
    ///
    /// assert_eq!(Path::new("foo/bar.txt"), iter.as_path());
    /// ```
    pub fn as_path(&self) -> &'a Path {
        self.inner.as_path()
    }
}

impl AsRef<Path> for Iter<'_> {
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

impl AsRef<OsStr> for Iter<'_> {
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a OsStr;

    fn next(&mut self) -> Option<&'a OsStr> {
        self.inner.next().map(Component::as_os_str)
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<&'a OsStr> {
        self.inner.next_back().map(Component::as_os_str)
    }
}

impl FusedIterator for Iter<'_> {}

impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Component<'a>> {
        while !self.finished() {
            match self.front {
                State::Prefix if self.prefix_len() > 0 => {
                    self.front = State::StartDir;
                    debug_assert!(self.prefix_len() <= self.path.len());
                    let raw = &self.path[..self.prefix_len()];
                    self.path = &self.path[self.prefix_len()..];
                    return Some(Component::Prefix(PrefixComponent {
                        raw: unsafe { u8_slice_as_os_str(raw) },
                        parsed: self.prefix.unwrap(),
                    }));
                }
                State::Prefix => {
                    self.front = State::StartDir;
                }
                State::StartDir => {
                    self.front = State::Body;
                    if self.has_physical_root {
                        debug_assert!(!self.path.is_empty());
                        self.path = &self.path[1..];
                        return Some(Component::RootDir);
                    } else if let Some(p) = self.prefix {
                        if p.has_implicit_root() && !p.is_verbatim() {
                            return Some(Component::RootDir);
                        }
                    } else if self.include_cur_dir() {
                        debug_assert!(!self.path.is_empty());
                        self.path = &self.path[1..];
                        return Some(Component::CurDir);
                    }
                }
                State::Body if !self.path.is_empty() => {
                    let (size, comp) = self.parse_next_component();
                    self.path = &self.path[size..];
                    if comp.is_some() {
                        return comp;
                    }
                }
                State::Body => {
                    self.front = State::Done;
                }
                State::Done => unreachable!(),
            }
        }
        None
    }
}

impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<Component<'a>> {
        while !self.finished() {
            match self.back {
                State::Body if self.path.len() > self.len_before_body() => {
                    let (size, comp) = self.parse_next_component_back();
                    self.path = &self.path[..self.path.len() - size];
                    if comp.is_some() {
                        return comp;
                    }
                }
                State::Body => {
                    self.back = State::StartDir;
                }
                State::StartDir => {
                    self.back = State::Prefix;
                    if self.has_physical_root {
                        self.path = &self.path[..self.path.len() - 1];
                        return Some(Component::RootDir);
                    } else if let Some(p) = self.prefix {
                        if p.has_implicit_root() && !p.is_verbatim() {
                            return Some(Component::RootDir);
                        }
                    } else if self.include_cur_dir() {
                        self.path = &self.path[..self.path.len() - 1];
                        return Some(Component::CurDir);
                    }
                }
                State::Prefix if self.prefix_len() > 0 => {
                    self.back = State::Done;
                    return Some(Component::Prefix(PrefixComponent {
                        raw: unsafe { u8_slice_as_os_str(self.path) },
                        parsed: self.prefix.unwrap(),
                    }));
                }
                State::Prefix => {
                    self.back = State::Done;
                    return None;
                }
                State::Done => unreachable!(),
            }
        }
        None
    }
}

impl FusedIterator for Components<'_> {}

impl<'a> cmp::PartialEq for Components<'a> {
    fn eq(&self, other: &Components<'a>) -> bool {
        Iterator::eq(self.clone(), other.clone())
    }
}

impl cmp::Eq for Components<'_> {}

impl<'a> cmp::PartialOrd for Components<'a> {
    fn partial_cmp(&self, other: &Components<'a>) -> Option<cmp::Ordering> {
        Iterator::partial_cmp(self.clone(), other.clone())
    }
}

impl cmp::Ord for Components<'_> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        Iterator::cmp(self.clone(), other.clone())
    }
}

/// An iterator over [`Path`] and its ancestors.
///
/// This `struct` is created by the [`ancestors`] method on [`Path`].
/// See its documentation for more.
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("/foo/bar");
///
/// for ancestor in path.ancestors() {
///     println!("{}", ancestor.display());
/// }
/// ```
///
/// [`ancestors`]: struct.Path.html#method.ancestors
/// [`Path`]: struct.Path.html
#[derive(Copy, Clone, Debug)]
pub struct Ancestors<'a> {
    next: Option<&'a Path>,
}

impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Path;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next;
        self.next = next.and_then(Path::parent);
        next
    }
}

impl FusedIterator for Ancestors<'_> {}

////////////////////////////////////////////////////////////////////////////////
// Basic types and traits
////////////////////////////////////////////////////////////////////////////////

/// An owned, mutable path (akin to [`String`]).
///
/// This type provides methods like [`push`] and [`set_extension`] that mutate
/// the path in place. It also implements [`Deref`] to [`Path`], meaning that
/// all methods on [`Path`] slices are available on `PathBuf` values as well.
///
/// [`String`]: ../string/struct.String.html
/// [`Path`]: struct.Path.html
/// [`push`]: struct.PathBuf.html#method.push
/// [`set_extension`]: struct.PathBuf.html#method.set_extension
/// [`Deref`]: ../ops/trait.Deref.html
///
/// More details about the overall approach can be found in
/// the [module documentation](index.html).
///
/// # Examples
///
/// You can use [`push`] to build up a `PathBuf` from
/// components:
///
/// ```
/// use std::path::PathBuf;
///
/// let mut path = PathBuf::new();
///
/// path.push(r"C:\");
/// path.push("windows");
/// path.push("system32");
///
/// path.set_extension("dll");
/// ```
///
/// However, [`push`] is best used for dynamic situations. This is a better way
/// to do this when you know all of the components ahead of time:
///
/// ```
/// use std::path::PathBuf;
///
/// let path: PathBuf = [r"C:\", "windows", "system32.dll"].iter().collect();
/// ```
///
/// We can still do better than this! Since these are all strings, we can use
/// `From::from`:
///
/// ```
/// use std::path::PathBuf;
///
/// let path = PathBuf::from(r"C:\windows\system32.dll");
/// ```
///
/// Which method works best depends on what kind of situation you're in.
#[derive(Clone)]
// FIXME:
// `PathBuf::as_mut_vec` current implementation relies
// on `PathBuf` being layout-compatible with `Vec<u8>`.
// When attribute privacy is implemented, `PathBuf` should be annotated as `#[repr(transparent)]`.
// Anyway, `PathBuf` representation and layout are considered implementation detail, are
// not documented and must not be relied upon.
pub struct PathBuf {
    inner: OsString,
}

impl PathBuf {
    fn as_mut_vec(&mut self) -> &mut Vec<u8> {
        unsafe { &mut *(self as *mut PathBuf as *mut Vec<u8>) }
    }

    /// Allocates an empty `PathBuf`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let path = PathBuf::new();
    /// ```
    pub fn new() -> PathBuf {
        PathBuf {
            inner: OsString::new(),
        }
    }

    /// Coerces to a [`Path`] slice.
    ///
    /// [`Path`]: struct.Path.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let p = PathBuf::from("/test");
    /// assert_eq!(Path::new("/test"), p.as_path());
    /// ```
    pub fn as_path(&self) -> &Path {
        self
    }

    /// Extends `self` with `path`.
    ///
    /// If `path` is absolute, it replaces the current path.
    ///
    /// On Windows:
    ///
    /// * if `path` has a root but no prefix (e.g., `\windows`), it
    ///   replaces everything except for the prefix (if any) of `self`.
    /// * if `path` has a prefix but no root, it replaces `self`.
    ///
    /// # Examples
    ///
    /// Pushing a relative path extends the existing path:
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut path = PathBuf::from("/tmp");
    /// path.push("file.bk");
    /// assert_eq!(path, PathBuf::from("/tmp/file.bk"));
    /// ```
    ///
    /// Pushing an absolute path replaces the existing path:
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut path = PathBuf::from("/tmp");
    /// path.push("/etc");
    /// assert_eq!(path, PathBuf::from("/etc"));
    /// ```
    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        self._push(path.as_ref())
    }

    fn _push(&mut self, path: &Path) {
        // in general, a separator is needed if the rightmost byte is not a separator
        let mut need_sep = self
            .as_mut_vec()
            .last()
            .map(|c| !is_sep_byte(*c))
            .unwrap_or(false);

        // in the special case of `C:` on Windows, do *not* add a separator
        {
            let comps = self.components();
            if comps.prefix_len() > 0
                && comps.prefix_len() == comps.path.len()
                && comps.prefix.unwrap().is_drive()
            {
                need_sep = false
            }
        }

        // absolute `path` replaces `self`
        if path.is_absolute() || path.prefix().is_some() {
            self.as_mut_vec().truncate(0);

        // `path` has a root but no prefix, e.g., `\windows` (Windows only)
        } else if path.has_root() {
            let prefix_len = self.components().prefix_remaining();
            self.as_mut_vec().truncate(prefix_len);

        // `path` is a pure relative path
        } else if need_sep {
            self.inner.push(MAIN_SEP_STR);
        }

        self.inner.push(path);
    }

    /// Truncates `self` to [`self.parent`].
    ///
    /// Returns `false` and does nothing if [`self.parent`] is [`None`].
    /// Otherwise, returns `true`.
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`self.parent`]: struct.PathBuf.html#method.parent
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/test/test.rs");
    ///
    /// p.pop();
    /// assert_eq!(Path::new("/test"), p);
    /// p.pop();
    /// assert_eq!(Path::new("/"), p);
    /// ```
    pub fn pop(&mut self) -> bool {
        match self.parent().map(|p| p.as_u8_slice().len()) {
            Some(len) => {
                self.as_mut_vec().truncate(len);
                true
            }
            None => false,
        }
    }

    /// Updates [`self.file_name`] to `file_name`.
    ///
    /// If [`self.file_name`] was [`None`], this is equivalent to pushing
    /// `file_name`.
    ///
    /// Otherwise it is equivalent to calling [`pop`] and then pushing
    /// `file_name`. The new path will be a sibling of the original path.
    /// (That is, it will have the same parent.)
    ///
    /// [`self.file_name`]: struct.PathBuf.html#method.file_name
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`pop`]: struct.PathBuf.html#method.pop
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut buf = PathBuf::from("/");
    /// assert!(buf.file_name() == None);
    /// buf.set_file_name("bar");
    /// assert!(buf == PathBuf::from("/bar"));
    /// assert!(buf.file_name().is_some());
    /// buf.set_file_name("baz.txt");
    /// assert!(buf == PathBuf::from("/baz.txt"));
    /// ```
    pub fn set_file_name<S: AsRef<OsStr>>(&mut self, file_name: S) {
        self._set_file_name(file_name.as_ref())
    }

    fn _set_file_name(&mut self, file_name: &OsStr) {
        if self.file_name().is_some() {
            let popped = self.pop();
            debug_assert!(popped);
        }
        self.push(file_name);
    }

    /// Updates [`self.extension`] to `extension`.
    ///
    /// Returns `false` and does nothing if [`self.file_name`] is [`None`],
    /// returns `true` and updates the extension otherwise.
    ///
    /// If [`self.extension`] is [`None`], the extension is added; otherwise
    /// it is replaced.
    ///
    /// [`self.file_name`]: struct.PathBuf.html#method.file_name
    /// [`self.extension`]: struct.PathBuf.html#method.extension
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/feel/the");
    ///
    /// p.set_extension("force");
    /// assert_eq!(Path::new("/feel/the.force"), p.as_path());
    ///
    /// p.set_extension("dark_side");
    /// assert_eq!(Path::new("/feel/the.dark_side"), p.as_path());
    /// ```
    pub fn set_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        self._set_extension(extension.as_ref())
    }

    fn _set_extension(&mut self, extension: &OsStr) -> bool {
        let file_stem = match self.file_stem() {
            None => return false,
            Some(f) => os_str_as_u8_slice(f),
        };

        // truncate until right after the file stem
        let end_file_stem = file_stem[file_stem.len()..].as_ptr() as usize;
        let start = os_str_as_u8_slice(&self.inner).as_ptr() as usize;
        let v = self.as_mut_vec();
        v.truncate(end_file_stem.wrapping_sub(start));

        // add the new extension, if any
        let new = os_str_as_u8_slice(extension);
        if !new.is_empty() {
            v.reserve_exact(new.len() + 1);
            v.push(b'.');
            v.extend_from_slice(new);
        }

        true
    }

    /// Consumes the `PathBuf`, yielding its internal [`OsString`] storage.
    ///
    /// [`OsString`]: ../ffi/struct.OsString.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let p = PathBuf::from("/the/head");
    /// let os_str = p.into_os_string();
    /// ```
    pub fn into_os_string(self) -> OsString {
        self.inner
    }

    /// Converts this `PathBuf` into a [boxed][`Box`] [`Path`].
    ///
    /// [`Box`]: ../../std/boxed/struct.Box.html
    /// [`Path`]: struct.Path.html
    pub fn into_boxed_path(self) -> Box<Path> {
        let rw = Box::into_raw(self.inner.into_boxed_os_str()) as *mut Path;
        unsafe { Box::from_raw(rw) }
    }
}

impl From<&Path> for Box<Path> {
    fn from(path: &Path) -> Box<Path> {
        let boxed: Box<OsStr> = path.inner.into();
        let rw = Box::into_raw(boxed) as *mut Path;
        unsafe { Box::from_raw(rw) }
    }
}

impl From<Box<Path>> for PathBuf {
    /// Converts a `Box<Path>` into a `PathBuf`
    ///
    /// This conversion does not allocate or copy memory.
    fn from(boxed: Box<Path>) -> PathBuf {
        boxed.into_path_buf()
    }
}

impl From<PathBuf> for Box<Path> {
    /// Converts a `PathBuf` into a `Box<Path>`
    ///
    /// This conversion currently should not allocate memory,
    /// but this behavior is not guaranteed on all platforms or in all future versions.
    fn from(p: PathBuf) -> Box<Path> {
        p.into_boxed_path()
    }
}

impl Clone for Box<Path> {
    #[inline]
    fn clone(&self) -> Self {
        self.to_path_buf().into_boxed_path()
    }
}

impl<T: ?Sized + AsRef<OsStr>> From<&T> for PathBuf {
    fn from(s: &T) -> PathBuf {
        PathBuf::from(s.as_ref().to_os_string())
    }
}

impl From<OsString> for PathBuf {
    /// Converts a `OsString` into a `PathBuf`
    ///
    /// This conversion does not allocate or copy memory.
    fn from(s: OsString) -> PathBuf {
        PathBuf { inner: s }
    }
}

impl From<PathBuf> for OsString {
    /// Converts a `PathBuf` into a `OsString`
    ///
    /// This conversion does not allocate or copy memory.
    fn from(path_buf: PathBuf) -> OsString {
        path_buf.inner
    }
}

impl From<String> for PathBuf {
    /// Converts a `String` into a `PathBuf`
    ///
    /// This conversion does not allocate or copy memory.
    fn from(s: String) -> PathBuf {
        PathBuf::from(OsString::from(s))
    }
}

impl FromStr for PathBuf {
    type Err = core::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PathBuf::from(s))
    }
}

impl<P: AsRef<Path>> iter::FromIterator<P> for PathBuf {
    fn from_iter<I: IntoIterator<Item = P>>(iter: I) -> PathBuf {
        let mut buf = PathBuf::new();
        buf.extend(iter);
        buf
    }
}

impl<P: AsRef<Path>> iter::Extend<P> for PathBuf {
    fn extend<I: IntoIterator<Item = P>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |p| self.push(p.as_ref()));
    }
}

impl fmt::Debug for PathBuf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

impl ops::Deref for PathBuf {
    type Target = Path;

    fn deref(&self) -> &Path {
        Path::new(&self.inner)
    }
}

impl Borrow<Path> for PathBuf {
    fn borrow(&self) -> &Path {
        self.deref()
    }
}

impl Default for PathBuf {
    fn default() -> Self {
        PathBuf::new()
    }
}

impl<'a> From<&'a Path> for Cow<'a, Path> {
    #[inline]
    fn from(s: &'a Path) -> Cow<'a, Path> {
        Cow::Borrowed(s)
    }
}

impl<'a> From<PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(s: PathBuf) -> Cow<'a, Path> {
        Cow::Owned(s)
    }
}

impl<'a> From<&'a PathBuf> for Cow<'a, Path> {
    #[inline]
    fn from(p: &'a PathBuf) -> Cow<'a, Path> {
        Cow::Borrowed(p.as_path())
    }
}

impl<'a> From<Cow<'a, Path>> for PathBuf {
    #[inline]
    fn from(p: Cow<'a, Path>) -> Self {
        p.into_owned()
    }
}

impl From<PathBuf> for Rc<Path> {
    /// Converts a `PathBuf` into an `Rc` by moving the `PathBuf` data into a new `Rc` buffer.
    #[inline]
    fn from(s: PathBuf) -> Rc<Path> {
        let rc: Rc<OsStr> = Rc::from(s.into_os_string());
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Path) }
    }
}

impl From<&Path> for Rc<Path> {
    /// Converts a `Path` into an `Rc` by copying the `Path` data into a new `Rc` buffer.
    #[inline]
    fn from(s: &Path) -> Rc<Path> {
        let rc: Rc<OsStr> = Rc::from(s.as_os_str());
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Path) }
    }
}

impl ToOwned for Path {
    type Owned = PathBuf;
    fn to_owned(&self) -> PathBuf {
        self.to_path_buf()
    }
}

impl cmp::PartialEq for PathBuf {
    fn eq(&self, other: &PathBuf) -> bool {
        self.components() == other.components()
    }
}

impl Hash for PathBuf {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_path().hash(h)
    }
}

impl cmp::Eq for PathBuf {}

impl cmp::PartialOrd for PathBuf {
    fn partial_cmp(&self, other: &PathBuf) -> Option<cmp::Ordering> {
        self.components().partial_cmp(other.components())
    }
}

impl cmp::Ord for PathBuf {
    fn cmp(&self, other: &PathBuf) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl AsRef<OsStr> for PathBuf {
    fn as_ref(&self) -> &OsStr {
        &self.inner[..]
    }
}

/// A slice of a path (akin to [`str`]).
///
/// This type supports a number of operations for inspecting a path, including
/// breaking the path into its components (separated by `/` on Unix and by either
/// `/` or `\` on Windows), extracting the file name, determining whether the path
/// is absolute, and so on.
///
/// This is an *unsized* type, meaning that it must always be used behind a
/// pointer like `&` or [`Box`]. For an owned version of this type,
/// see [`PathBuf`].
///
/// [`str`]: ../primitive.str.html
/// [`Box`]: ../boxed/struct.Box.html
/// [`PathBuf`]: struct.PathBuf.html
///
/// More details about the overall approach can be found in
/// the [module documentation](index.html).
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use std::ffi::OsStr;
///
/// // Note: this example does work on Windows
/// let path = Path::new("./foo/bar.txt");
///
/// let parent = path.parent();
/// assert_eq!(parent, Some(Path::new("./foo")));
///
/// let file_stem = path.file_stem();
/// assert_eq!(file_stem, Some(OsStr::new("bar")));
///
/// let extension = path.extension();
/// assert_eq!(extension, Some(OsStr::new("txt")));
/// ```
// FIXME:
// `Path::new` current implementation relies
// on `Path` being layout-compatible with `OsStr`.
// When attribute privacy is implemented, `Path` should be annotated as `#[repr(transparent)]`.
// Anyway, `Path` representation and layout are considered implementation detail, are
// not documented and must not be relied upon.
pub struct Path {
    inner: OsStr,
}

/// An error returned from [`Path::strip_prefix`][`strip_prefix`] if the prefix
/// was not found.
///
/// This `struct` is created by the [`strip_prefix`] method on [`Path`].
/// See its documentation for more.
///
/// [`strip_prefix`]: struct.Path.html#method.strip_prefix
/// [`Path`]: struct.Path.html
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StripPrefixError(());

impl Path {
    // The following (private!) function allows construction of a path from a u8
    // slice, which is only safe when it is known to follow the OsStr encoding.
    unsafe fn from_u8_slice(s: &[u8]) -> &Path {
        Path::new(u8_slice_as_os_str(s))
    }
    // The following (private!) function reveals the byte encoding used for OsStr.
    fn as_u8_slice(&self) -> &[u8] {
        os_str_as_u8_slice(&self.inner)
    }

    /// Directly wraps a string slice as a `Path` slice.
    ///
    /// This is a cost-free conversion.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// Path::new("foo.txt");
    /// ```
    ///
    /// You can create `Path`s from `String`s, or even other `Path`s:
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let string = String::from("foo.txt");
    /// let from_string = Path::new(&string);
    /// let from_path = Path::new(&from_string);
    /// assert_eq!(from_string, from_path);
    /// ```
    pub fn new<S: AsRef<OsStr> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    /// Yields the underlying [`OsStr`] slice.
    ///
    /// [`OsStr`]: ../ffi/struct.OsStr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let os_str = Path::new("foo.txt").as_os_str();
    /// assert_eq!(os_str, std::ffi::OsStr::new("foo.txt"));
    /// ```
    pub fn as_os_str(&self) -> &OsStr {
        &self.inner
    }

    /// Yields a [`&str`] slice if the `Path` is valid unicode.
    ///
    /// This conversion may entail doing a check for UTF-8 validity.
    /// Note that validation is performed because non-UTF-8 strings are
    /// perfectly valid for some OS.
    ///
    /// [`&str`]: ../primitive.str.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.txt");
    /// assert_eq!(path.to_str(), Some("foo.txt"));
    /// ```
    pub fn to_str(&self) -> Option<&str> {
        self.inner.to_str()
    }

    /// Converts a `Path` to a [`Cow<str>`].
    ///
    /// Any non-Unicode sequences are replaced with
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD].
    ///
    /// [`Cow<str>`]: ../borrow/enum.Cow.html
    /// [U+FFFD]: ../char/constant.REPLACEMENT_CHARACTER.html
    ///
    /// # Examples
    ///
    /// Calling `to_string_lossy` on a `Path` with valid unicode:
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.txt");
    /// assert_eq!(path.to_string_lossy(), "foo.txt");
    /// ```
    ///
    /// Had `path` contained invalid unicode, the `to_string_lossy` call might
    /// have returned `"fo�.txt"`.
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        self.inner.to_string_lossy()
    }

    /// Converts a `Path` to an owned [`PathBuf`].
    ///
    /// [`PathBuf`]: struct.PathBuf.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path_buf = Path::new("foo.txt").to_path_buf();
    /// assert_eq!(path_buf, std::path::PathBuf::from("foo.txt"));
    /// ```
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.inner.to_os_string())
    }

    /// Returns `true` if the `Path` is absolute, i.e., if it is independent of
    /// the current directory.
    ///
    /// * On Unix, a path is absolute if it starts with the root, so
    /// `is_absolute` and [`has_root`] are equivalent.
    ///
    /// * On Windows, a path is absolute if it has a prefix and starts with the
    /// root: `c:\windows` is absolute, while `c:temp` and `\temp` are not.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert!(!Path::new("foo.txt").is_absolute());
    /// ```
    ///
    /// [`has_root`]: #method.has_root
    #[allow(deprecated)]
    pub fn is_absolute(&self) -> bool {
        if cfg!(target_os = "redox") {
            // FIXME: Allow Redox prefixes
            self.has_root() || has_redox_scheme(self.as_u8_slice())
        } else {
            // self.has_root() && (cfg!(unix) || self.prefix().is_some())
            self.has_root()
        }
    }

    /// Returns `true` if the `Path` is relative, i.e., not absolute.
    ///
    /// See [`is_absolute`]'s documentation for more details.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert!(Path::new("foo.txt").is_relative());
    /// ```
    ///
    /// [`is_absolute`]: #method.is_absolute
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    fn prefix(&self) -> Option<Prefix<'_>> {
        self.components().prefix
    }

    /// Returns `true` if the `Path` has a root.
    ///
    /// * On Unix, a path has a root if it begins with `/`.
    ///
    /// * On Windows, a path has a root if it:
    ///     * has no prefix and begins with a separator, e.g., `\windows`
    ///     * has a prefix followed by a separator, e.g., `c:\windows` but not `c:windows`
    ///     * has any non-disk prefix, e.g., `\\server\share`
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert!(Path::new("/etc/passwd").has_root());
    /// ```
    pub fn has_root(&self) -> bool {
        self.components().has_root()
    }

    /// Returns the `Path` without its final component, if there is one.
    ///
    /// Returns [`None`] if the path terminates in a root or prefix.
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/foo/bar");
    /// let parent = path.parent().unwrap();
    /// assert_eq!(parent, Path::new("/foo"));
    ///
    /// let grand_parent = parent.parent().unwrap();
    /// assert_eq!(grand_parent, Path::new("/"));
    /// assert_eq!(grand_parent.parent(), None);
    /// ```
    pub fn parent(&self) -> Option<&Path> {
        let mut comps = self.components();
        let comp = comps.next_back();
        comp.and_then(|p| match p {
            Component::Normal(_) | Component::CurDir | Component::ParentDir => {
                Some(comps.as_path())
            }
            _ => None,
        })
    }

    /// Produces an iterator over `Path` and its ancestors.
    ///
    /// The iterator will yield the `Path` that is returned if the [`parent`] method is used zero
    /// or more times. That means, the iterator will yield `&self`, `&self.parent().unwrap()`,
    /// `&self.parent().unwrap().parent().unwrap()` and so on. If the [`parent`] method returns
    /// [`None`], the iterator will do likewise. The iterator will always yield at least one value,
    /// namely `&self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut ancestors = Path::new("/foo/bar").ancestors();
    /// assert_eq!(ancestors.next(), Some(Path::new("/foo/bar")));
    /// assert_eq!(ancestors.next(), Some(Path::new("/foo")));
    /// assert_eq!(ancestors.next(), Some(Path::new("/")));
    /// assert_eq!(ancestors.next(), None);
    /// ```
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    /// [`parent`]: struct.Path.html#method.parent
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors { next: Some(&self) }
    }

    /// Returns the final component of the `Path`, if there is one.
    ///
    /// If the path is a normal file, this is the file name. If it's the path of a directory, this
    /// is the directory name.
    ///
    /// Returns [`None`] if the path terminates in `..`.
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    /// use std::ffi::OsStr;
    ///
    /// assert_eq!(Some(OsStr::new("bin")), Path::new("/usr/bin/").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("tmp/foo.txt").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.//").file_name());
    /// assert_eq!(None, Path::new("foo.txt/..").file_name());
    /// assert_eq!(None, Path::new("/").file_name());
    /// ```
    pub fn file_name(&self) -> Option<&OsStr> {
        self.components().next_back().and_then(|p| match p {
            Component::Normal(p) => Some(p.as_ref()),
            _ => None,
        })
    }

    /// Returns a path that, when joined onto `base`, yields `self`.
    ///
    /// # Errors
    ///
    /// If `base` is not a prefix of `self` (i.e., [`starts_with`]
    /// returns `false`), returns [`Err`].
    ///
    /// [`starts_with`]: #method.starts_with
    /// [`Err`]: ../../std/result/enum.Result.html#variant.Err
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/test/haha/foo.txt");
    ///
    /// assert_eq!(path.strip_prefix("/"), Ok(Path::new("test/haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test"), Ok(Path::new("haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test/"), Ok(Path::new("haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test/haha/foo.txt"), Ok(Path::new("")));
    /// assert_eq!(path.strip_prefix("/test/haha/foo.txt/"), Ok(Path::new("")));
    /// assert_eq!(path.strip_prefix("test").is_ok(), false);
    /// assert_eq!(path.strip_prefix("/haha").is_ok(), false);
    ///
    /// let prefix = PathBuf::from("/test/");
    /// assert_eq!(path.strip_prefix(prefix), Ok(Path::new("haha/foo.txt")));
    /// ```
    pub fn strip_prefix<P>(&self, base: P) -> Result<&Path, StripPrefixError>
    where
        P: AsRef<Path>,
    {
        self._strip_prefix(base.as_ref())
    }

    fn _strip_prefix(&self, base: &Path) -> Result<&Path, StripPrefixError> {
        iter_after(self.components(), base.components())
            .map(|c| c.as_path())
            .ok_or(StripPrefixError(()))
    }

    /// Determines whether `base` is a prefix of `self`.
    ///
    /// Only considers whole path components to match.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/etc/passwd");
    ///
    /// assert!(path.starts_with("/etc"));
    /// assert!(path.starts_with("/etc/"));
    /// assert!(path.starts_with("/etc/passwd"));
    /// assert!(path.starts_with("/etc/passwd/"));
    ///
    /// assert!(!path.starts_with("/e"));
    /// ```
    pub fn starts_with<P: AsRef<Path>>(&self, base: P) -> bool {
        self._starts_with(base.as_ref())
    }

    fn _starts_with(&self, base: &Path) -> bool {
        iter_after(self.components(), base.components()).is_some()
    }

    /// Determines whether `child` is a suffix of `self`.
    ///
    /// Only considers whole path components to match.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/etc/passwd");
    ///
    /// assert!(path.ends_with("passwd"));
    /// ```
    pub fn ends_with<P: AsRef<Path>>(&self, child: P) -> bool {
        self._ends_with(child.as_ref())
    }

    fn _ends_with(&self, child: &Path) -> bool {
        iter_after(self.components().rev(), child.components().rev()).is_some()
    }

    /// Extracts the stem (non-extension) portion of [`self.file_name`].
    ///
    /// [`self.file_name`]: struct.Path.html#method.file_name
    ///
    /// The stem is:
    ///
    /// * [`None`], if there is no file name;
    /// * The entire file name if there is no embedded `.`;
    /// * The entire file name if the file name begins with `.` and has no other `.`s within;
    /// * Otherwise, the portion of the file name before the final `.`
    ///
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.rs");
    ///
    /// assert_eq!("foo", path.file_stem().unwrap());
    /// ```
    pub fn file_stem(&self) -> Option<&OsStr> {
        self.file_name()
            .map(split_file_at_dot)
            .and_then(|(before, after)| before.or(after))
    }

    /// Extracts the extension of [`self.file_name`], if possible.
    ///
    /// The extension is:
    ///
    /// * [`None`], if there is no file name;
    /// * [`None`], if there is no embedded `.`;
    /// * [`None`], if the file name begins with `.` and has no other `.`s within;
    /// * Otherwise, the portion of the file name after the final `.`
    ///
    /// [`self.file_name`]: struct.Path.html#method.file_name
    /// [`None`]: ../../std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.rs");
    ///
    /// assert_eq!("rs", path.extension().unwrap());
    /// ```
    pub fn extension(&self) -> Option<&OsStr> {
        self.file_name()
            .map(split_file_at_dot)
            .and_then(|(before, after)| before.and(after))
    }

    /// Creates an owned [`PathBuf`] with `path` adjoined to `self`.
    ///
    /// See [`PathBuf::push`] for more details on what it means to adjoin a path.
    ///
    /// [`PathBuf`]: struct.PathBuf.html
    /// [`PathBuf::push`]: struct.PathBuf.html#method.push
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// assert_eq!(Path::new("/etc").join("passwd"), PathBuf::from("/etc/passwd"));
    /// ```
    #[must_use]
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self._join(path.as_ref())
    }

    fn _join(&self, path: &Path) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.push(path);
        buf
    }

    /// Creates an owned [`PathBuf`] like `self` but with the given file name.
    ///
    /// See [`PathBuf::set_file_name`] for more details.
    ///
    /// [`PathBuf`]: struct.PathBuf.html
    /// [`PathBuf::set_file_name`]: struct.PathBuf.html#method.set_file_name
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/tmp/foo.txt");
    /// assert_eq!(path.with_file_name("bar.txt"), PathBuf::from("/tmp/bar.txt"));
    ///
    /// let path = Path::new("/tmp");
    /// assert_eq!(path.with_file_name("var"), PathBuf::from("/var"));
    /// ```
    pub fn with_file_name<S: AsRef<OsStr>>(&self, file_name: S) -> PathBuf {
        self._with_file_name(file_name.as_ref())
    }

    fn _with_file_name(&self, file_name: &OsStr) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_file_name(file_name);
        buf
    }

    /// Creates an owned [`PathBuf`] like `self` but with the given extension.
    ///
    /// See [`PathBuf::set_extension`] for more details.
    ///
    /// [`PathBuf`]: struct.PathBuf.html
    /// [`PathBuf::set_extension`]: struct.PathBuf.html#method.set_extension
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("foo.rs");
    /// assert_eq!(path.with_extension("txt"), PathBuf::from("foo.txt"));
    /// ```
    pub fn with_extension<S: AsRef<OsStr>>(&self, extension: S) -> PathBuf {
        self._with_extension(extension.as_ref())
    }

    fn _with_extension(&self, extension: &OsStr) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_extension(extension);
        buf
    }

    /// Produces an iterator over the [`Component`]s of the path.
    ///
    /// When parsing the path, there is a small amount of normalization:
    ///
    /// * Repeated separators are ignored, so `a/b` and `a//b` both have
    ///   `a` and `b` as components.
    ///
    /// * Occurrences of `.` are normalized away, except if they are at the
    ///   beginning of the path. For example, `a/./b`, `a/b/`, `a/b/.` and
    ///   `a/b` all have `a` and `b` as components, but `./a/b` starts with
    ///   an additional [`CurDir`] component.
    ///
    /// * A trailing slash is normalized away, `/a/b` and `/a/b/` are equivalent.
    ///
    /// Note that no other normalization takes place; in particular, `a/c`
    /// and `a/b/../c` are distinct, to account for the possibility that `b`
    /// is a symbolic link (so its parent isn't `a`).
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, Component};
    /// use std::ffi::OsStr;
    ///
    /// let mut components = Path::new("/tmp/foo.txt").components();
    ///
    /// assert_eq!(components.next(), Some(Component::RootDir));
    /// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("tmp"))));
    /// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("foo.txt"))));
    /// assert_eq!(components.next(), None)
    /// ```
    ///
    /// [`Component`]: enum.Component.html
    /// [`CurDir`]: enum.Component.html#variant.CurDir
    pub fn components(&self) -> Components<'_> {
        let prefix = parse_prefix(self.as_os_str());
        Components {
            path: self.as_u8_slice(),
            prefix,
            has_physical_root: has_physical_root(self.as_u8_slice(), prefix)
                || has_redox_scheme(self.as_u8_slice()),
            front: State::Prefix,
            back: State::Body,
        }
    }

    /// Produces an iterator over the path's components viewed as [`OsStr`]
    /// slices.
    ///
    /// For more information about the particulars of how the path is separated
    /// into components, see [`components`].
    ///
    /// [`components`]: #method.components
    /// [`OsStr`]: ../ffi/struct.OsStr.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{self, Path};
    /// use std::ffi::OsStr;
    ///
    /// let mut it = Path::new("/tmp/foo.txt").iter();
    /// assert_eq!(it.next(), Some(OsStr::new(&path::MAIN_SEPARATOR.to_string())));
    /// assert_eq!(it.next(), Some(OsStr::new("tmp")));
    /// assert_eq!(it.next(), Some(OsStr::new("foo.txt")));
    /// assert_eq!(it.next(), None)
    /// ```
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            inner: self.components(),
        }
    }

    /// Returns an object that implements [`Display`] for safely printing paths
    /// that may contain non-Unicode data.
    ///
    /// [`Display`]: ../fmt/trait.Display.html
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/tmp/foo.rs");
    ///
    /// println!("{}", path.display());
    /// ```
    pub fn display(&self) -> Display<'_> {
        Display { path: self }
    }

    /// Converts a [`Box<Path>`][`Box`] into a [`PathBuf`] without copying or
    /// allocating.
    ///
    /// [`Box`]: ../../std/boxed/struct.Box.html
    /// [`PathBuf`]: struct.PathBuf.html
    pub fn into_path_buf(self: Box<Path>) -> PathBuf {
        let rw = Box::into_raw(self) as *mut OsStr;
        let inner = unsafe { Box::from_raw(rw) };
        PathBuf {
            inner: OsString::from(inner),
        }
    }
}

impl AsRef<OsStr> for Path {
    fn as_ref(&self) -> &OsStr {
        &self.inner
    }
}

impl fmt::Debug for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

/// Helper struct for safely printing paths with [`format!`] and `{}`.
///
/// A [`Path`] might contain non-Unicode data. This `struct` implements the
/// [`Display`] trait in a way that mitigates that. It is created by the
/// [`display`][`Path::display`] method on [`Path`].
///
/// # Examples
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("/tmp/foo.rs");
///
/// println!("{}", path.display());
/// ```
///
/// [`Display`]: ../../std/fmt/trait.Display.html
/// [`format!`]: ../../std/macro.format.html
/// [`Path`]: struct.Path.html
/// [`Path::display`]: struct.Path.html#method.display
pub struct Display<'a> {
    path: &'a Path,
}

impl fmt::Debug for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.path, f)
    }
}

impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.path.inner.display(f)
    }
}

impl cmp::PartialEq for Path {
    fn eq(&self, other: &Path) -> bool {
        self.components().eq(other.components())
    }
}

impl Hash for Path {
    fn hash<H: Hasher>(&self, h: &mut H) {
        for component in self.components() {
            component.hash(h);
        }
    }
}

impl cmp::Eq for Path {}

impl cmp::PartialOrd for Path {
    fn partial_cmp(&self, other: &Path) -> Option<cmp::Ordering> {
        self.components().partial_cmp(other.components())
    }
}

impl cmp::Ord for Path {
    fn cmp(&self, other: &Path) -> cmp::Ordering {
        self.components().cmp(other.components())
    }
}

impl AsRef<Path> for Path {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl AsRef<Path> for OsStr {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for Cow<'_, OsStr> {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for OsString {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for str {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for String {
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

impl AsRef<Path> for PathBuf {
    fn as_ref(&self) -> &Path {
        self
    }
}

impl<'a> IntoIterator for &'a PathBuf {
    type Item = &'a OsStr;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a Path {
    type Item = &'a OsStr;
    type IntoIter = Iter<'a>;
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

macro_rules! impl_cmp {
    ($lhs:ty, $rhs: ty) => {
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <Path as PartialEq>::eq(self, other)
            }
        }

        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <Path as PartialEq>::eq(self, other)
            }
        }

        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self, other)
            }
        }

        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self, other)
            }
        }
    };
}

impl_cmp!(PathBuf, Path);
impl_cmp!(PathBuf, &'a Path);
impl_cmp!(Cow<'a, Path>, Path);
impl_cmp!(Cow<'a, Path>, &'b Path);
impl_cmp!(Cow<'a, Path>, PathBuf);

macro_rules! impl_cmp_os_str {
    ($lhs:ty, $rhs: ty) => {
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <Path as PartialEq>::eq(self, other.as_ref())
            }
        }

        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <Path as PartialEq>::eq(self.as_ref(), other)
            }
        }

        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self, other.as_ref())
            }
        }

        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self.as_ref(), other)
            }
        }
    };
}

impl_cmp_os_str!(PathBuf, OsStr);
impl_cmp_os_str!(PathBuf, &'a OsStr);
impl_cmp_os_str!(PathBuf, Cow<'a, OsStr>);
impl_cmp_os_str!(PathBuf, OsString);
impl_cmp_os_str!(Path, OsStr);
impl_cmp_os_str!(Path, &'a OsStr);
impl_cmp_os_str!(Path, Cow<'a, OsStr>);
impl_cmp_os_str!(Path, OsString);
impl_cmp_os_str!(&'a Path, OsStr);
impl_cmp_os_str!(&'a Path, Cow<'b, OsStr>);
impl_cmp_os_str!(&'a Path, OsString);
impl_cmp_os_str!(Cow<'a, Path>, OsStr);
impl_cmp_os_str!(Cow<'a, Path>, &'b OsStr);
impl_cmp_os_str!(Cow<'a, Path>, OsString);

#[cfg(test)]
mod tests {
    use super::*;

    use alloc::format;
    use alloc::rc::Rc;
    use alloc::string::ToString;

    macro_rules! t(
        ($path:expr, iter: $iter:expr) => (
            {
                let path = Path::new($path);

                // Forward iteration
                let comps = path.iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect::<Vec<String>>();
                let exp: &[&str] = &$iter;
                let exps = exp.iter().map(|s| s.to_string()).collect::<Vec<String>>();
                assert!(comps == exps, "iter: Expected {:?}, found {:?}",
                        exps, comps);

                // Reverse iteration
                let comps = Path::new($path).iter().rev()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect::<Vec<String>>();
                let exps = exps.into_iter().rev().collect::<Vec<String>>();
                assert!(comps == exps, "iter().rev(): Expected {:?}, found {:?}",
                        exps, comps);
            }
        );

        ($path:expr, has_root: $has_root:expr, is_absolute: $is_absolute:expr) => (
            {
                let path = Path::new($path);

                let act_root = path.has_root();
                assert!(act_root == $has_root, "has_root: Expected {:?}, found {:?}",
                        $has_root, act_root);

                let act_abs = path.is_absolute();
                assert!(act_abs == $is_absolute, "is_absolute: Expected {:?}, found {:?}",
                        $is_absolute, act_abs);
            }
        );

        ($path:expr, parent: $parent:expr, file_name: $file:expr) => (
            {
                let path = Path::new($path);

                let parent = path.parent().map(|p| p.to_str().unwrap());
                let exp_parent: Option<&str> = $parent;
                assert!(parent == exp_parent, "parent: Expected {:?}, found {:?}",
                        exp_parent, parent);

                let file = path.file_name().map(|p| p.to_str().unwrap());
                let exp_file: Option<&str> = $file;
                assert!(file == exp_file, "file_name: Expected {:?}, found {:?}",
                        exp_file, file);
            }
        );

        ($path:expr, file_stem: $file_stem:expr, extension: $extension:expr) => (
            {
                let path = Path::new($path);

                let stem = path.file_stem().map(|p| p.to_str().unwrap());
                let exp_stem: Option<&str> = $file_stem;
                assert!(stem == exp_stem, "file_stem: Expected {:?}, found {:?}",
                        exp_stem, stem);

                let ext = path.extension().map(|p| p.to_str().unwrap());
                let exp_ext: Option<&str> = $extension;
                assert!(ext == exp_ext, "extension: Expected {:?}, found {:?}",
                        exp_ext, ext);
            }
        );

        ($path:expr, iter: $iter:expr,
                     has_root: $has_root:expr, is_absolute: $is_absolute:expr,
                     parent: $parent:expr, file_name: $file:expr,
                     file_stem: $file_stem:expr, extension: $extension:expr) => (
            {
                t!($path, iter: $iter);
                t!($path, has_root: $has_root, is_absolute: $is_absolute);
                t!($path, parent: $parent, file_name: $file);
                t!($path, file_stem: $file_stem, extension: $extension);
            }
        );
    );

    #[test]
    fn into() {
        use alloc::borrow::Cow;

        let static_path = Path::new("/home/foo");
        let static_cow_path: Cow<'static, Path> = static_path.into();
        let pathbuf = PathBuf::from("/home/foo");

        {
            let path: &Path = &pathbuf;
            let borrowed_cow_path: Cow<'_, Path> = path.into();

            assert_eq!(static_cow_path, borrowed_cow_path);
        }

        let owned_cow_path: Cow<'static, Path> = pathbuf.into();

        assert_eq!(static_cow_path, owned_cow_path);
    }

    #[test]
    #[cfg(unix)]
    pub fn test_decompositions_unix() {
        t!("",
        iter: [],
        has_root: false,
        is_absolute: false,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("/",
        iter: ["/"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("/foo",
        iter: ["/", "foo"],
        has_root: true,
        is_absolute: true,
        parent: Some("/"),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("/foo/",
        iter: ["/", "foo"],
        has_root: true,
        is_absolute: true,
        parent: Some("/"),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/bar",
        iter: ["foo", "bar"],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("/foo/bar",
        iter: ["/", "foo", "bar"],
        has_root: true,
        is_absolute: true,
        parent: Some("/foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("///foo///",
        iter: ["/", "foo"],
        has_root: true,
        is_absolute: true,
        parent: Some("/"),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("///foo///bar",
        iter: ["/", "foo", "bar"],
        has_root: true,
        is_absolute: true,
        parent: Some("///foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("./.",
        iter: ["."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("/..",
        iter: ["/", ".."],
        has_root: true,
        is_absolute: true,
        parent: Some("/"),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("../",
        iter: [".."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo/.",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/..",
        iter: ["foo", ".."],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo/./",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/./bar",
        iter: ["foo", "bar"],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("foo/../",
        iter: ["foo", ".."],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo/../bar",
        iter: ["foo", "..", "bar"],
        has_root: false,
        is_absolute: false,
        parent: Some("foo/.."),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("./a",
        iter: [".", "a"],
        has_root: false,
        is_absolute: false,
        parent: Some("."),
        file_name: Some("a"),
        file_stem: Some("a"),
        extension: None
        );

        t!(".",
        iter: ["."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("./",
        iter: ["."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("a/b",
        iter: ["a", "b"],
        has_root: false,
        is_absolute: false,
        parent: Some("a"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );

        t!("a//b",
        iter: ["a", "b"],
        has_root: false,
        is_absolute: false,
        parent: Some("a"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );

        t!("a/./b",
        iter: ["a", "b"],
        has_root: false,
        is_absolute: false,
        parent: Some("a"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );

        t!("a/b/c",
        iter: ["a", "b", "c"],
        has_root: false,
        is_absolute: false,
        parent: Some("a/b"),
        file_name: Some("c"),
        file_stem: Some("c"),
        extension: None
        );

        t!(".foo",
        iter: [".foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some(".foo"),
        file_stem: Some(".foo"),
        extension: None
        );
    }

    #[test]
    #[cfg(windows)]
    pub fn test_decompositions_windows() {
        t!("",
        iter: [],
        has_root: false,
        is_absolute: false,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("/",
        iter: ["\\"],
        has_root: true,
        is_absolute: false,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\",
        iter: ["\\"],
        has_root: true,
        is_absolute: false,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("c:",
        iter: ["c:"],
        has_root: false,
        is_absolute: false,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("c:\\",
        iter: ["c:", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("c:/",
        iter: ["c:", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("/foo",
        iter: ["\\", "foo"],
        has_root: true,
        is_absolute: false,
        parent: Some("/"),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("/foo/",
        iter: ["\\", "foo"],
        has_root: true,
        is_absolute: false,
        parent: Some("/"),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/bar",
        iter: ["foo", "bar"],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("/foo/bar",
        iter: ["\\", "foo", "bar"],
        has_root: true,
        is_absolute: false,
        parent: Some("/foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("///foo///",
        iter: ["\\", "foo"],
        has_root: true,
        is_absolute: false,
        parent: Some("/"),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("///foo///bar",
        iter: ["\\", "foo", "bar"],
        has_root: true,
        is_absolute: false,
        parent: Some("///foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("./.",
        iter: ["."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("/..",
        iter: ["\\", ".."],
        has_root: true,
        is_absolute: false,
        parent: Some("/"),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("../",
        iter: [".."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo/.",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/..",
        iter: ["foo", ".."],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo/./",
        iter: ["foo"],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: Some("foo"),
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo/./bar",
        iter: ["foo", "bar"],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("foo/../",
        iter: ["foo", ".."],
        has_root: false,
        is_absolute: false,
        parent: Some("foo"),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("foo/../bar",
        iter: ["foo", "..", "bar"],
        has_root: false,
        is_absolute: false,
        parent: Some("foo/.."),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("./a",
        iter: [".", "a"],
        has_root: false,
        is_absolute: false,
        parent: Some("."),
        file_name: Some("a"),
        file_stem: Some("a"),
        extension: None
        );

        t!(".",
        iter: ["."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("./",
        iter: ["."],
        has_root: false,
        is_absolute: false,
        parent: Some(""),
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("a/b",
        iter: ["a", "b"],
        has_root: false,
        is_absolute: false,
        parent: Some("a"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );

        t!("a//b",
        iter: ["a", "b"],
        has_root: false,
        is_absolute: false,
        parent: Some("a"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );

        t!("a/./b",
        iter: ["a", "b"],
        has_root: false,
        is_absolute: false,
        parent: Some("a"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );

        t!("a/b/c",
           iter: ["a", "b", "c"],
           has_root: false,
           is_absolute: false,
           parent: Some("a/b"),
           file_name: Some("c"),
           file_stem: Some("c"),
           extension: None);

        t!("a\\b\\c",
        iter: ["a", "b", "c"],
        has_root: false,
        is_absolute: false,
        parent: Some("a\\b"),
        file_name: Some("c"),
        file_stem: Some("c"),
        extension: None
        );

        t!("\\a",
        iter: ["\\", "a"],
        has_root: true,
        is_absolute: false,
        parent: Some("\\"),
        file_name: Some("a"),
        file_stem: Some("a"),
        extension: None
        );

        t!("c:\\foo.txt",
        iter: ["c:", "\\", "foo.txt"],
        has_root: true,
        is_absolute: true,
        parent: Some("c:\\"),
        file_name: Some("foo.txt"),
        file_stem: Some("foo"),
        extension: Some("txt")
        );

        t!("\\\\server\\share\\foo.txt",
        iter: ["\\\\server\\share", "\\", "foo.txt"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\server\\share\\"),
        file_name: Some("foo.txt"),
        file_stem: Some("foo"),
        extension: Some("txt")
        );

        t!("\\\\server\\share",
        iter: ["\\\\server\\share", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\server",
        iter: ["\\", "server"],
        has_root: true,
        is_absolute: false,
        parent: Some("\\"),
        file_name: Some("server"),
        file_stem: Some("server"),
        extension: None
        );

        t!("\\\\?\\bar\\foo.txt",
        iter: ["\\\\?\\bar", "\\", "foo.txt"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\?\\bar\\"),
        file_name: Some("foo.txt"),
        file_stem: Some("foo"),
        extension: Some("txt")
        );

        t!("\\\\?\\bar",
        iter: ["\\\\?\\bar"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\",
        iter: ["\\\\?\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\UNC\\server\\share\\foo.txt",
        iter: ["\\\\?\\UNC\\server\\share", "\\", "foo.txt"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\?\\UNC\\server\\share\\"),
        file_name: Some("foo.txt"),
        file_stem: Some("foo"),
        extension: Some("txt")
        );

        t!("\\\\?\\UNC\\server",
        iter: ["\\\\?\\UNC\\server"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\UNC\\",
        iter: ["\\\\?\\UNC\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\C:\\foo.txt",
        iter: ["\\\\?\\C:", "\\", "foo.txt"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\?\\C:\\"),
        file_name: Some("foo.txt"),
        file_stem: Some("foo"),
        extension: Some("txt")
        );

        t!("\\\\?\\C:\\",
        iter: ["\\\\?\\C:", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\C:",
        iter: ["\\\\?\\C:"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\foo/bar",
        iter: ["\\\\?\\foo/bar"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\C:/foo",
        iter: ["\\\\?\\C:/foo"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\.\\foo\\bar",
        iter: ["\\\\.\\foo", "\\", "bar"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\.\\foo\\"),
        file_name: Some("bar"),
        file_stem: Some("bar"),
        extension: None
        );

        t!("\\\\.\\foo",
        iter: ["\\\\.\\foo", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\.\\foo/bar",
        iter: ["\\\\.\\foo/bar", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\.\\foo\\bar/baz",
        iter: ["\\\\.\\foo", "\\", "bar", "baz"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\.\\foo\\bar"),
        file_name: Some("baz"),
        file_stem: Some("baz"),
        extension: None
        );

        t!("\\\\.\\",
        iter: ["\\\\.\\", "\\"],
        has_root: true,
        is_absolute: true,
        parent: None,
        file_name: None,
        file_stem: None,
        extension: None
        );

        t!("\\\\?\\a\\b\\",
        iter: ["\\\\?\\a", "\\", "b"],
        has_root: true,
        is_absolute: true,
        parent: Some("\\\\?\\a\\"),
        file_name: Some("b"),
        file_stem: Some("b"),
        extension: None
        );
    }

    #[test]
    pub fn test_stem_ext() {
        t!("foo",
        file_stem: Some("foo"),
        extension: None
        );

        t!("foo.",
        file_stem: Some("foo"),
        extension: Some("")
        );

        t!(".foo",
        file_stem: Some(".foo"),
        extension: None
        );

        t!("foo.txt",
        file_stem: Some("foo"),
        extension: Some("txt")
        );

        t!("foo.bar.txt",
        file_stem: Some("foo.bar"),
        extension: Some("txt")
        );

        t!("foo.bar.",
        file_stem: Some("foo.bar"),
        extension: Some("")
        );

        t!(".", file_stem: None, extension: None);

        t!("..", file_stem: None, extension: None);

        t!("", file_stem: None, extension: None);
    }

    #[test]
    pub fn test_push() {
        macro_rules! tp(
            ($path:expr, $push:expr, $expected:expr) => ( {
                let mut actual = PathBuf::from($path);
                actual.push($push);
                assert!(actual.to_str() == Some($expected),
                        "pushing {:?} onto {:?}: Expected {:?}, got {:?}",
                        $push, $path, $expected, actual.to_str().unwrap());
            });
        );

        if cfg!(unix) || cfg!(all(target_env = "sgx", target_vendor = "fortanix")) {
            tp!("", "foo", "foo");
            tp!("foo", "bar", "foo/bar");
            tp!("foo/", "bar", "foo/bar");
            tp!("foo//", "bar", "foo//bar");
            tp!("foo/.", "bar", "foo/./bar");
            tp!("foo./.", "bar", "foo././bar");
            tp!("foo", "", "foo/");
            tp!("foo", ".", "foo/.");
            tp!("foo", "..", "foo/..");
            tp!("foo", "/", "/");
            tp!("/foo/bar", "/", "/");
            tp!("/foo/bar", "/baz", "/baz");
            tp!("/foo/bar", "./baz", "/foo/bar/./baz");
        } else {
            tp!("", "foo", "foo");
            tp!("foo", "bar", r"foo\bar");
            tp!("foo/", "bar", r"foo/bar");
            tp!(r"foo\", "bar", r"foo\bar");
            tp!("foo//", "bar", r"foo//bar");
            tp!(r"foo\\", "bar", r"foo\\bar");
            tp!("foo/.", "bar", r"foo/.\bar");
            tp!("foo./.", "bar", r"foo./.\bar");
            tp!(r"foo\.", "bar", r"foo\.\bar");
            tp!(r"foo.\.", "bar", r"foo.\.\bar");
            tp!("foo", "", "foo\\");
            tp!("foo", ".", r"foo\.");
            tp!("foo", "..", r"foo\..");
            tp!("foo", "/", "/");
            tp!("foo", r"\", r"\");
            tp!("/foo/bar", "/", "/");
            tp!(r"\foo\bar", r"\", r"\");
            tp!("/foo/bar", "/baz", "/baz");
            tp!("/foo/bar", r"\baz", r"\baz");
            tp!("/foo/bar", "./baz", r"/foo/bar\./baz");
            tp!("/foo/bar", r".\baz", r"/foo/bar\.\baz");

            tp!("c:\\", "windows", "c:\\windows");
            tp!("c:", "windows", "c:windows");

            tp!("a\\b\\c", "d", "a\\b\\c\\d");
            tp!("\\a\\b\\c", "d", "\\a\\b\\c\\d");
            tp!("a\\b", "c\\d", "a\\b\\c\\d");
            tp!("a\\b", "\\c\\d", "\\c\\d");
            tp!("a\\b", ".", "a\\b\\.");
            tp!("a\\b", "..\\c", "a\\b\\..\\c");
            tp!("a\\b", "C:a.txt", "C:a.txt");
            tp!("a\\b", "C:\\a.txt", "C:\\a.txt");
            tp!("C:\\a", "C:\\b.txt", "C:\\b.txt");
            tp!("C:\\a\\b\\c", "C:d", "C:d");
            tp!("C:a\\b\\c", "C:d", "C:d");
            tp!("C:", r"a\b\c", r"C:a\b\c");
            tp!("C:", r"..\a", r"C:..\a");
            tp!(
                "\\\\server\\share\\foo",
                "bar",
                "\\\\server\\share\\foo\\bar"
            );
            tp!("\\\\server\\share\\foo", "C:baz", "C:baz");
            tp!("\\\\?\\C:\\a\\b", "C:c\\d", "C:c\\d");
            tp!("\\\\?\\C:a\\b", "C:c\\d", "C:c\\d");
            tp!("\\\\?\\C:\\a\\b", "C:\\c\\d", "C:\\c\\d");
            tp!("\\\\?\\foo\\bar", "baz", "\\\\?\\foo\\bar\\baz");
            tp!(
                "\\\\?\\UNC\\server\\share\\foo",
                "bar",
                "\\\\?\\UNC\\server\\share\\foo\\bar"
            );
            tp!("\\\\?\\UNC\\server\\share", "C:\\a", "C:\\a");
            tp!("\\\\?\\UNC\\server\\share", "C:a", "C:a");

            // Note: modified from old path API
            tp!("\\\\?\\UNC\\server", "foo", "\\\\?\\UNC\\server\\foo");

            tp!(
                "C:\\a",
                "\\\\?\\UNC\\server\\share",
                "\\\\?\\UNC\\server\\share"
            );
            tp!("\\\\.\\foo\\bar", "baz", "\\\\.\\foo\\bar\\baz");
            tp!("\\\\.\\foo\\bar", "C:a", "C:a");
            // again, not sure about the following, but I'm assuming \\.\ should be verbatim
            tp!("\\\\.\\foo", "..\\bar", "\\\\.\\foo\\..\\bar");

            tp!("\\\\?\\C:", "foo", "\\\\?\\C:\\foo"); // this is a weird one
        }
    }

    #[test]
    pub fn test_pop() {
        macro_rules! tp(
            ($path:expr, $expected:expr, $output:expr) => ( {
                let mut actual = PathBuf::from($path);
                let output = actual.pop();
                assert!(actual.to_str() == Some($expected) && output == $output,
                        "popping from {:?}: Expected {:?}/{:?}, got {:?}/{:?}",
                        $path, $expected, $output,
                        actual.to_str().unwrap(), output);
            });
        );

        tp!("", "", false);
        tp!("/", "/", false);
        tp!("foo", "", true);
        tp!(".", "", true);
        tp!("/foo", "/", true);
        tp!("/foo/bar", "/foo", true);
        tp!("foo/bar", "foo", true);
        tp!("foo/.", "", true);
        tp!("foo//bar", "foo", true);

        if cfg!(windows) {
            tp!("a\\b\\c", "a\\b", true);
            tp!("\\a", "\\", true);
            tp!("\\", "\\", false);

            tp!("C:\\a\\b", "C:\\a", true);
            tp!("C:\\a", "C:\\", true);
            tp!("C:\\", "C:\\", false);
            tp!("C:a\\b", "C:a", true);
            tp!("C:a", "C:", true);
            tp!("C:", "C:", false);
            tp!("\\\\server\\share\\a\\b", "\\\\server\\share\\a", true);
            tp!("\\\\server\\share\\a", "\\\\server\\share\\", true);
            tp!("\\\\server\\share", "\\\\server\\share", false);
            tp!("\\\\?\\a\\b\\c", "\\\\?\\a\\b", true);
            tp!("\\\\?\\a\\b", "\\\\?\\a\\", true);
            tp!("\\\\?\\a", "\\\\?\\a", false);
            tp!("\\\\?\\C:\\a\\b", "\\\\?\\C:\\a", true);
            tp!("\\\\?\\C:\\a", "\\\\?\\C:\\", true);
            tp!("\\\\?\\C:\\", "\\\\?\\C:\\", false);
            tp!(
                "\\\\?\\UNC\\server\\share\\a\\b",
                "\\\\?\\UNC\\server\\share\\a",
                true
            );
            tp!(
                "\\\\?\\UNC\\server\\share\\a",
                "\\\\?\\UNC\\server\\share\\",
                true
            );
            tp!(
                "\\\\?\\UNC\\server\\share",
                "\\\\?\\UNC\\server\\share",
                false
            );
            tp!("\\\\.\\a\\b\\c", "\\\\.\\a\\b", true);
            tp!("\\\\.\\a\\b", "\\\\.\\a\\", true);
            tp!("\\\\.\\a", "\\\\.\\a", false);

            tp!("\\\\?\\a\\b\\", "\\\\?\\a\\", true);
        }
    }

    #[test]
    pub fn test_set_file_name() {
        macro_rules! tfn(
                ($path:expr, $file:expr, $expected:expr) => ( {
                let mut p = PathBuf::from($path);
                p.set_file_name($file);
                assert!(p.to_str() == Some($expected),
                        "setting file name of {:?} to {:?}: Expected {:?}, got {:?}",
                        $path, $file, $expected,
                        p.to_str().unwrap());
            });
        );

        tfn!("foo", "foo", "foo");
        tfn!("foo", "bar", "bar");
        tfn!("foo", "", "");
        tfn!("", "foo", "foo");
        if cfg!(unix) || cfg!(all(target_env = "sgx", target_vendor = "fortanix")) {
            tfn!(".", "foo", "./foo");
            tfn!("foo/", "bar", "bar");
            tfn!("foo/.", "bar", "bar");
            tfn!("..", "foo", "../foo");
            tfn!("foo/..", "bar", "foo/../bar");
            tfn!("/", "foo", "/foo");
        } else {
            tfn!(".", "foo", r".\foo");
            tfn!(r"foo\", "bar", r"bar");
            tfn!(r"foo\.", "bar", r"bar");
            tfn!("..", "foo", r"..\foo");
            tfn!(r"foo\..", "bar", r"foo\..\bar");
            tfn!(r"\", "foo", r"\foo");
        }
    }

    #[test]
    pub fn test_set_extension() {
        macro_rules! tfe(
                ($path:expr, $ext:expr, $expected:expr, $output:expr) => ( {
                let mut p = PathBuf::from($path);
                let output = p.set_extension($ext);
                assert!(p.to_str() == Some($expected) && output == $output,
                        "setting extension of {:?} to {:?}: Expected {:?}/{:?}, got {:?}/{:?}",
                        $path, $ext, $expected, $output,
                        p.to_str().unwrap(), output);
            });
        );

        tfe!("foo", "txt", "foo.txt", true);
        tfe!("foo.bar", "txt", "foo.txt", true);
        tfe!("foo.bar.baz", "txt", "foo.bar.txt", true);
        tfe!(".test", "txt", ".test.txt", true);
        tfe!("foo.txt", "", "foo", true);
        tfe!("foo", "", "foo", true);
        tfe!("", "foo", "", false);
        tfe!(".", "foo", ".", false);
        tfe!("foo/", "bar", "foo.bar", true);
        tfe!("foo/.", "bar", "foo.bar", true);
        tfe!("..", "foo", "..", false);
        tfe!("foo/..", "bar", "foo/..", false);
        tfe!("/", "foo", "/", false);
    }

    #[test]
    fn test_eq_receivers() {
        use alloc::borrow::Cow;

        let borrowed: &Path = Path::new("foo/bar");
        let mut owned: PathBuf = PathBuf::new();
        owned.push("foo");
        owned.push("bar");
        let borrowed_cow: Cow<'_, Path> = borrowed.into();
        let owned_cow: Cow<'_, Path> = owned.clone().into();

        macro_rules! t {
            ($($current:expr),+) => {
                $(
                    assert_eq!($current, borrowed);
                    assert_eq!($current, owned);
                    assert_eq!($current, borrowed_cow);
                    assert_eq!($current, owned_cow);
                )+
            }
        }

        t!(borrowed, owned, borrowed_cow, owned_cow);
    }

    #[test]
    pub fn test_compare() {
        use core::hash::{BuildHasher, Hash, Hasher};
        use hashbrown::hash_map::DefaultHashBuilder;

        fn hash<T: Hash>(t: T) -> u64 {
            let mut s = DefaultHashBuilder::new().build_hasher();
            t.hash(&mut s);
            s.finish()
        }

        macro_rules! tc(
            ($path1:expr, $path2:expr, eq: $eq:expr,
             starts_with: $starts_with:expr, ends_with: $ends_with:expr,
             relative_from: $relative_from:expr) => ({
                 let path1 = Path::new($path1);
                 let path2 = Path::new($path2);

                 let eq = path1 == path2;
                 assert!(eq == $eq, "{:?} == {:?}, expected {:?}, got {:?}",
                         $path1, $path2, $eq, eq);
                 assert!($eq == (hash(path1) == hash(path2)),
                         "{:?} == {:?}, expected {:?}, got {} and {}",
                         $path1, $path2, $eq, hash(path1), hash(path2));

                 let starts_with = path1.starts_with(path2);
                 assert!(starts_with == $starts_with,
                         "{:?}.starts_with({:?}), expected {:?}, got {:?}", $path1, $path2,
                         $starts_with, starts_with);

                 let ends_with = path1.ends_with(path2);
                 assert!(ends_with == $ends_with,
                         "{:?}.ends_with({:?}), expected {:?}, got {:?}", $path1, $path2,
                         $ends_with, ends_with);

                 let relative_from = path1.strip_prefix(path2)
                                          .map(|p| p.to_str().unwrap())
                                          .ok();
                 let exp: Option<&str> = $relative_from;
                 assert!(relative_from == exp,
                         "{:?}.strip_prefix({:?}), expected {:?}, got {:?}",
                         $path1, $path2, exp, relative_from);
            });
        );

        tc!("", "",
        eq: true,
        starts_with: true,
        ends_with: true,
        relative_from: Some("")
        );

        tc!("foo", "",
        eq: false,
        starts_with: true,
        ends_with: true,
        relative_from: Some("foo")
        );

        tc!("", "foo",
        eq: false,
        starts_with: false,
        ends_with: false,
        relative_from: None
        );

        tc!("foo", "foo",
        eq: true,
        starts_with: true,
        ends_with: true,
        relative_from: Some("")
        );

        tc!("foo/", "foo",
        eq: true,
        starts_with: true,
        ends_with: true,
        relative_from: Some("")
        );

        tc!("foo/bar", "foo",
        eq: false,
        starts_with: true,
        ends_with: false,
        relative_from: Some("bar")
        );

        tc!("foo/bar/baz", "foo/bar",
        eq: false,
        starts_with: true,
        ends_with: false,
        relative_from: Some("baz")
        );

        tc!("foo/bar", "foo/bar/baz",
        eq: false,
        starts_with: false,
        ends_with: false,
        relative_from: None
        );

        tc!("./foo/bar/", ".",
        eq: false,
        starts_with: true,
        ends_with: false,
        relative_from: Some("foo/bar")
        );

        if cfg!(windows) {
            tc!(r"C:\src\rust\cargo-test\test\Cargo.toml",
            r"c:\src\rust\cargo-test\test",
            eq: false,
            starts_with: true,
            ends_with: false,
            relative_from: Some("Cargo.toml")
            );

            tc!(r"c:\foo", r"C:\foo",
            eq: true,
            starts_with: true,
            ends_with: true,
            relative_from: Some("")
            );
        }
    }

    #[test]
    fn test_components_debug() {
        let path = Path::new("/tmp");

        let mut components = path.components();

        let expected = "Components([RootDir, Normal(\"tmp\")])";
        let actual = format!("{:?}", components);
        assert_eq!(expected, actual);

        let _ = components.next().unwrap();
        let expected = "Components([Normal(\"tmp\")])";
        let actual = format!("{:?}", components);
        assert_eq!(expected, actual);

        let _ = components.next().unwrap();
        let expected = "Components([])";
        let actual = format!("{:?}", components);
        assert_eq!(expected, actual);
    }

    #[cfg(unix)]
    #[test]
    fn test_iter_debug() {
        let path = Path::new("/tmp");

        let mut iter = path.iter();

        let expected = "Iter([\"/\", \"tmp\"])";
        let actual = format!("{:?}", iter);
        assert_eq!(expected, actual);

        let _ = iter.next().unwrap();
        let expected = "Iter([\"tmp\"])";
        let actual = format!("{:?}", iter);
        assert_eq!(expected, actual);

        let _ = iter.next().unwrap();
        let expected = "Iter([])";
        let actual = format!("{:?}", iter);
        assert_eq!(expected, actual);
    }

    #[test]
    fn into_boxed() {
        let orig: &str = "some/sort/of/path";
        let path = Path::new(orig);
        let boxed: Box<Path> = Box::from(path);
        let path_buf = path.to_owned().into_boxed_path().into_path_buf();
        assert_eq!(path, &*boxed);
        assert_eq!(&*boxed, &*path_buf);
        assert_eq!(&*path_buf, path);
    }

    #[test]
    fn display_format_flags() {
        assert_eq!(format!("a{:#<5}b", Path::new("").display()), "a#####b");
        assert_eq!(format!("a{:#<5}b", Path::new("a").display()), "aa####b");
    }

    #[test]
    fn into_rc() {
        let orig = "hello/world";
        let path = Path::new(orig);
        let rc: Rc<Path> = Rc::from(path);

        assert_eq!(&*rc, path);

        let rc2: Rc<Path> = Rc::from(path.to_owned());

        assert_eq!(&*rc2, path);
    }

    #[test]
    fn is_absolute() {
        let p = Path::new("/tmp/foo/bar.txt");
        let mut components = p.components();

        assert!(p.is_absolute());

        let root = PathBuf::from("/");
        assert!(root.is_absolute());
    }
}
