#![feature(asm)]
#![feature(global_asm)]

#![cfg_attr(not(test), no_std)]

#[macro_use]
pub mod macros;

pub mod sp;
pub mod asm;
pub mod regs;
pub mod vmsa;

pub use sp::SP;
pub use regs::*;
pub use vmsa::*;
pub use asm::*;

/// 現在の例外レベルを返す.
///
/// # 安全性
///
/// この関数はEL >= 1 の時にしか呼び出すことはできない.
#[inline(always)]
pub fn current_el() -> u8 {
    ((unsafe { CurrentEL.get() } & 0b1100) >> 2) as u8
}

/// SPSel値を返す.
#[inline(always)]
pub fn sp_sel() -> u8 {
    unsafe {
        SPSel.get_value(SPSel::SP) as u8
    }
}

/// 現在実行中のコアを返す.
///
/// # 安全性
///
/// この関数はEL >= 1 の時にしか呼び出すことはできない.
pub fn affinity() -> usize {
    unsafe {
        MPIDR_EL1.get_value(MPIDR_EL1::Aff0) as usize
    }
}
