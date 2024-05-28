use crate::common::IO_BASE;
use core::time::Duration;

use volatile::prelude::*;
use volatile::{ReadVolatile, Volatile};

/// ARMシステムタイマーレジスタの基底アドレス.
const TIMER_REG_BASE: usize = IO_BASE + 0x3000;
// タイマー周波数: 1MHz
//const TIMER_FREQ: u64 = 1_000_000;

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

    /// 現在から `t` 時間後にタイマー1のマッチが発生するように設定する。
    /// タイマー1の割り込みが有効かつIRQがアンマスクの場合、`t` 時間後に
    /// タイマー割り込みが発行される。
    pub fn tick_in(&mut self, t: Duration) {
        let now = self.registers.CLO.read();
        let fire = now.wrapping_add(t.as_micros() as u32);
        self.registers.COMPARE[1].write(fire);
        self.registers.CS.write(0b10);
    }
}

/// 現在時刻を返す.
pub fn current_time() -> Duration {
    Timer::new().read()
}

/// `t` 時間が過ぎるまでスピンする.
pub fn spin_sleep(t: Duration) {
    let start = current_time();
    while current_time() < start + t {}
}

/// 現在から `t` 時間後にタイマー1のマッチが発生するように設定する。
/// タイマー1の割り込みが有効かつIRQがアンマスクの場合、`t` 時間後に
/// タイマー割り込みが発行される。
pub fn tick_in(t: Duration) {
    let mut timer = Timer::new();
    timer.tick_in(t);
}
