use crate::common::IO_BASE;
use core::time::Duration;

use volatile::prelude::*;
use volatile::{ReadVolatile, Volatile};

/// ARMシステムタイマーレジスタの基底アドレス.
const TIMER_REG_BASE: usize = IO_BASE + 0x3000;

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    CS: Volatile<u32>,
    CLO: ReadVolatile<u32>,
    CHI: ReadVolatile<u32>,
    COMPARE: [Volatile<u32>; 4],
}

/// Raspberry PiのARMシステムタイマー.
pub struct Timer {
    registers: &'static mut Registers,
}

impl Timer {
    /// `Timer`の新規インスタンスを返す.
    pub fn new() -> Timer {
        Timer {
            registers: unsafe { &mut *(TIMER_REG_BASE as *mut Registers) },
        }
    }

    /// システムタイマーのカウンタを読み取り、Durationを返す.
    /// `CLO`と`CHI`を合わせることで経過したマイクロ秒数を表す
    /// ことができる。
    pub fn read(&self) -> Duration {
        let counts = ((self.registers.CHI.read() as u64) << 32) |
                      (self.registers.CLO.read() as u64);
        Duration::from_micros(counts)
    }
}

/// 現在時刻を返す.
pub fn current_time() -> Duration {
    Timer::new().read()
}

/// Spins until `t` duration have passed.
pub fn spin_sleep(t: Duration) {
    let start = current_time();
    while current_time() < start + t {}
}
