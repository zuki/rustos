use core::time::Duration;

use aarch64::{CNTFRQ_EL0, CNTP_CTL_EL0, CNTP_TVAL_EL0};
use volatile::prelude::*;
use volatile::{Volatile, ReadVolatile, WriteVolatile, Reserved};

use crate::common::NCORES;

const INT_BASE: usize = 0x40000000;

/// コア割り込みソース (QA7: 4.10)
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum LocalInterrupt {
    // Lab 5 1.C
    CNTPSIRQ = 0,
    CNTPNSIRQ = 1,
    CNTHPIRQ = 2,
    CNTVIRQ = 3,
    MAILBOX0 = 4,
    MAILBOX1 = 5,
    MAILBOX2 = 6,
    MAILBOX3 = 7,
    GPU = 8,
    PMU = 9,
    AXIOUTSTNADING = 10,
    LOCALTIMER = 11,
}

impl LocalInterrupt {
    pub const MAX: usize = 12;

    pub fn iter() -> impl Iterator<Item = LocalInterrupt> {
        (0..LocalInterrupt::MAX).map(|n| LocalInterrupt::from(n))
    }
}

impl From<usize> for LocalInterrupt {
    fn from(irq: usize) -> LocalInterrupt {
        // Lab 5 1.C
        use LocalInterrupt::*;
        match irq {
            0 => CNTPSIRQ,
            1 => CNTPNSIRQ,
            2 => CNTHPIRQ,
            3 => CNTVIRQ,
            4 => MAILBOX0,
            5 => MAILBOX1,
            6 => MAILBOX2,
            7 => MAILBOX3,
            8 => GPU,
            9 => PMU,
            10 => AXIOUTSTNADING,
            11 => LOCALTIMER,
            _ => panic!("Unknown local irq: {}", irq),
        }
    }
}

/// BCM2837 ローカルペリフェラルレジスタ (QA7: Chapter 4)
#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    // Lab 5 1.C
    CONTROL: WriteVolatile<u32>,
    __r0: Reserved<u32>,
    CORE_TIMER_PRESCALER: WriteVolatile<u32>,
    GPU_INTERRUPT_ROUTING: WriteVolatile<u32>,
    PMU_SET: WriteVolatile<u32>,
    PMU_CLEAR: WriteVolatile<u32>,
    __r1: Reserved<u32>,
    CORE_TIMER_LS: Volatile<u32>,
    CORE_TIMER_MS: Volatile<u32>,
    LOCAL_INTERRUPT_ROUTING_0_7: WriteVolatile<u32>,
    LOCAL_INTERRUPT_ROUTING_8_15: WriteVolatile<u32>,
    __r2: [Reserved<u32>; 2],
    LOCAL_TIMER_CONTROL: Volatile<u32>,
    LOCAL_TIMER_CLEAR: WriteVolatile<u32>,
    __r3: Reserved<u32>,
    CORE_TIMER_CONTROL: [WriteVolatile<u32>; NCORES],
    CORE_MAILBOX_CONTROL: [WriteVolatile<u32>; NCORES],
    CORE_IRQ_SOURCE: [ReadVolatile<u32>; NCORES],
    CORE_FIQ_SOURCE: [ReadVolatile<u32>; NCORES],
}

pub struct LocalController {
    core: usize,
    registers: &'static mut Registers,
}

impl LocalController {
    /// Returns a new handle to the interrupt controller.
    pub fn new(core: usize) -> LocalController {
        LocalController {
            core,
            registers: unsafe { &mut *(INT_BASE as *mut Registers) },
        }
    }

    pub fn enable_local_timer(&mut self) {
        // Lab 5 1.C
        unsafe { CNTP_CTL_EL0.set(CNTP_CTL_EL0.get() | CNTP_CTL_EL0::ENABLE) };
        self.registers.CORE_TIMER_CONTROL[self.core].write(1 << LocalInterrupt::CNTPNSIRQ as u32);
    }

    pub fn is_pending(&self, int: LocalInterrupt) -> bool {
        // Lab 5 1.C
        self.registers.CORE_IRQ_SOURCE[self.core].has_mask(1 << int as u32)
    }

    pub fn tick_in(&mut self, t: Duration) {
        // Lab 5 1.C
        // See timer: 3.1 to 3.3
        let freq = unsafe { CNTFRQ_EL0.get() };
        let fire = (freq / 1_000) * t.as_millis() as u64;
        unsafe {CNTP_TVAL_EL0.set(fire) };
    }
}

pub fn local_tick_in(core: usize, t: Duration) {
    LocalController::new(core).tick_in(t);
}
