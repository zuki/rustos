use crate::common::IO_BASE;

use volatile::prelude::*;
use volatile::{ReadVolatile, Volatile};

const INT_BASE: usize = IO_BASE + 0xB000 + 0x200;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Interrupt {
    Timer1 = 1,
    Timer3 = 3,
    Usb = 9,
    Gpio0 = 49,
    Gpio1 = 50,
    Gpio2 = 51,
    Gpio3 = 52,
    Uart = 57,
}

impl Interrupt {
    pub const MAX: usize = 8;

    pub fn iter() -> impl Iterator<Item = Interrupt> {
        use Interrupt::*;
        [Timer1, Timer3, Usb, Gpio0, Gpio1, Gpio2, Gpio3, Uart]
            .iter()
            .map(|int| *int)
    }
}

impl From<usize> for Interrupt {
    fn from(irq: usize) -> Interrupt {
        use Interrupt::*;
        match irq {
            1 => Timer1,
            3 => Timer3,
            9 => Usb,
            49 => Gpio0,
            50 => Gpio1,
            51 => Gpio2,
            52 => Gpio3,
            57 => Uart,
            _ => panic!("Unknown irq: {}", irq),
        }
    }
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    // FIXME: Fill me in.
    IRQ_BASIC_PENDING: ReadVolatile<u32>,
    IRQ_PENDING_1: ReadVolatile<u32>,
    IRQ_PENDING_2: ReadVolatile<u32>,
    FIQ_CONTROL: Volatile<u32>,
    ENABLE_IRQ_1: Volatile<u32>,
    ENABLE_IRQ_2: Volatile<u32>,
    ENABLE_BASIC_IRQ: Volatile<u32>,
    DISABLE_IRQ_1: Volatile<u32>,
    DISABLE_IRQ_2: Volatile<u32>,
    DISABLE_BASIC_IRQ: Volatile<u32>,
}

/// 割り込みコントローラー. 割り込みの有効化、無効化、保留中の
/// 割り込みのチェックに使用される。
pub struct Controller {
    registers: &'static mut Registers,
}

impl Controller {
    /// 割り込みコントローラへの新規ハンドルを返す.
    pub fn new() -> Controller {
        Controller {
            registers: unsafe { &mut *(INT_BASE as *mut Registers) },
        }
    }

    /// 番号 `int` の割り込みを有効にする.
    pub fn enable(&mut self, int: Interrupt) {
        let index = int as u32;
        if index < 32 {
            self.registers.ENABLE_IRQ_1.write(
                self.registers.ENABLE_IRQ_1.read() | (1 << index));
        } else {
            self.registers.ENABLE_IRQ_2.write(
                self.registers.ENABLE_IRQ_2.read() | (1 << (index - 32)));
        }
    }

    /// 番号 `int` の割り込みを無効にする.
    pub fn disable(&mut self, int: Interrupt) {
        let index = int as u32;
        if index < 32 {
            self.registers.DISABLE_IRQ_1.write(
                self.registers.DISABLE_IRQ_1.read() | (1 << index));
        } else {
            self.registers.DISABLE_IRQ_2.write(
                self.registers.DISABLE_IRQ_2.read() | (1 << (index - 32)));
        }
    }

    /// 番号 `int` の割り込みが保留中の場合は `true` を返す。
    /// そうでなければ `false` を返す.
    pub fn is_pending(&self, int: Interrupt) -> bool {
        let index = int as u32;
        if index < 32 {
            (self.registers.IRQ_PENDING_1.read()) & (1 << index) != 0
        } else {
            (self.registers.IRQ_PENDING_2.read()) & (1 << (index - 32)) != 0
        }
    }

    /// USB割り込みをFIQ割り込みとして有効化
    pub fn enable_fiq(&mut self, _int: Interrupt) {
        // Lab 5 2.B
        self.disable(Interrupt::Usb);
        self.registers.FIQ_CONTROL.write(0x89 as u32);  // USB: 9 + enable flag
    }
}
