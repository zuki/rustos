/// メモリにレイアウトされている通りの生の`ATAG`.
#[repr(C)]
pub struct Atag {
    pub dwords: u32,
    pub tag: u32,
    pub kind: Kind,
}

impl Atag {
    pub const NONE: u32 = 0x00000000;
    pub const CORE: u32 = 0x54410001;
    pub const MEM: u32 = 0x54410002;
    pub const VIDEOTEXT: u32 = 0x54410003;
    pub const RAMDISK: u32 = 0x54410004;
    pub const INITRD2: u32 = 0x54420005;
    pub const SERIAL: u32 = 0x54410006;
    pub const REVISION: u32 = 0x54410007;
    pub const VIDEOLFB: u32 = 0x54410008;
    pub const CMDLINE: u32 = 0x54410009;

    /// FIXME: Returns the ATAG following `self`, if there is one.
    pub fn next(&self) -> Option<&Atag> {
        if self.tag == Atag::NONE {
            None
        } else {
            let mut atag = (self as *const Atag) as *const u32;
            atag = unsafe { atag.add(self.dwords as usize) };
            let next = unsafe { &*(atag as *const Atag) };
            Some(next)
        }
    }
}

/// ATAGのバリアント.
#[repr(C)]
pub union Kind {
    pub core: Core,
    pub mem: Mem,
    pub cmd: Cmd,
}

/// A `CORE` ATAG.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Core {
    pub flags: u32,
    pub page_size: u32,
    pub root_dev: u32,
}

/// A `MEM` ATAG.
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Mem {
    pub size: u32,
    pub start: u32,
}

/// A `CMDLINE` ATAG.
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct Cmd {
    /// コマンドライン文字列の最初のバイト
    pub cmd: u8,
}
