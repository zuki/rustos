use shim::{const_assert_eq, const_assert_size};

// we only support 64-bit
const_assert_size!(usize, 64 / 8);

use core::time::Duration;
pub use pi::common::*;

// 1 << PAGE_ALIGN = PAGE_SIZE
pub const PAGE_ALIGN: usize = 16;
pub const PAGE_SIZE: usize = 64 * 1024;
pub const PAGE_MASK: usize = !(PAGE_SIZE - 1);

// USER_MASK_BITS = 1 << 34 = 0x4_0000_0000 = 16GB
pub const USER_MASK_BITS: usize = 34;
// KERNE_MAKS_BITS = 1 << 31 = 0x8000_0000 = 2GB
pub const KERNEL_MASK_BITS: usize = 31;

pub const USER_IMG_BASE: usize = 0xffff_ffff_c000_0000;
const_assert_eq!(
    USER_IMG_BASE,
    ((1 << USER_MASK_BITS) - 1) << (64 - USER_MASK_BITS)
);
// 0xFFFF_FFFF_FFFF_0000
pub const USER_STACK_BASE: usize = core::usize::MAX & PAGE_MASK;
// 1GB
pub const USER_MAX_VM_SIZE: usize = 0x4000_0000;
// 0xFFFF_FFFF_FFFF_FFFF
pub const USER_MAX_VM: usize = (USER_IMG_BASE - 1 + USER_MAX_VM_SIZE);

const_assert_eq!(USER_IMG_BASE.wrapping_add(USER_MAX_VM_SIZE), 0);

pub const KERN_STACK_BASE: usize = 0x80_000;
pub const KERN_STACK_ALIGN: usize = PAGE_ALIGN;
pub const KERN_STACK_SIZE: usize = PAGE_SIZE;

/// The `tick` time.
// FIXME: When you're ready, change this to something more reasonable.
//pub const TICK: Duration = Duration::from_secs(2);
pub const TICK: Duration = Duration::from_secs(2);

// Match this value with `HZ` in `timer.h`
pub const USPI_TIMER_HZ: usize = 10;

// Match this value with `USPI_FRAME_BUFFER_SIZE` in `uspi.h`
pub const USPI_FRAME_BUFFER_SIZE: u32 = 1600;
pub const MTU: u32 = 1500;
