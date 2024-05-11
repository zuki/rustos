use core::fmt;

#[repr(C)]
#[derive(Default, Copy, Clone, Debug)]
pub struct TrapFrame {
    pub elr: u64,
    pub spsr: u64,
    pub sp: u64,
    pub tpidr: u64,
    pub qn: [u128; 32],
    pub xn: [u64; 31],
    pub zero: u64,
}
