use crate::atags::raw;
pub use crate::atags::raw::{Core, Mem};

/// An ATAG.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Atag {
    Core(raw::Core),
    Mem(raw::Mem),
    Cmd(&'static str),
    Unknown(u32),
    None,
}

impl Atag {
    /// これが`Core` ATAGの場合は`Some`を、それ以外は`None`を返す.
    pub fn core(self) -> Option<Core> {
        match self {
            Atag::Core(s) => Some(s),
            _ => None,
        }
    }

    /// これが`Mem` ATAGの場合は`Some`を、それ以外は`None`を返す.
    pub fn mem(self) -> Option<Mem> {
        match self {
            Atag::Mem(s) => Some(s),
            _ => None,
        }
    }

    /// これが`Cmd` ATAGの場合はコマンドライン文字列をもつ`Some`を、
    /// それ以外は`None`を返す.
    pub fn cmd(self) -> Option<&'static str> {
        match self {
            Atag::Cmd(s) => Some(s),
            _ => None,
        }
    }
}

fn parse_cmd(cmd: &raw::Cmd) -> &'static str {
    let mut size: usize = 0;
    let ptr = &cmd.cmd as *const u8;

    while unsafe { *(ptr.add(size)) } != b'\0' {
        size += 1;
    }

    let buf = unsafe { core::slice::from_raw_parts(&cmd.cmd, size) };
    unsafe { core::str::from_utf8_unchecked(buf) }
}

// FIXME: Implement `From<&raw::Atag> for `Atag`.
impl From<&'static raw::Atag> for Atag {
    fn from(atag: &'static raw::Atag) -> Atag {
        // FIXME: Complete the implementation below.

        unsafe {
            match (atag.tag, &atag.kind) {
                (raw::Atag::CORE, &raw::Kind { core }) => Atag::Core(core),
                (raw::Atag::MEM, &raw::Kind { mem }) => Atag::Mem(mem),
                (raw::Atag::CMDLINE, &raw::Kind { ref cmd }) => Atag::Cmd(parse_cmd(cmd)),
                (raw::Atag::NONE, _) => Atag::None,
                (id, _) => Atag::Unknown(id),
            }
        }
    }
}
