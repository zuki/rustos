use core::marker::PhantomData;

use crate::common::{states, GPIO_BASE};
use volatile::prelude::*;
use volatile::{ReadVolatile, Reserved, Volatile, WriteVolatile};

/// GPIOの代替機能.
#[repr(u8)]
pub enum Function {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    FSEL: [Volatile<u32>; 6],
    __r0: Reserved<u32>,
    SET: [WriteVolatile<u32>; 2],
    __r1: Reserved<u32>,
    CLR: [WriteVolatile<u32>; 2],
    __r2: Reserved<u32>,
    LEV: [ReadVolatile<u32>; 2],
    __r3: Reserved<u32>,
    EDS: [Volatile<u32>; 2],
    __r4: Reserved<u32>,
    REN: [Volatile<u32>; 2],
    __r5: Reserved<u32>,
    FEN: [Volatile<u32>; 2],
    __r6: Reserved<u32>,
    HEN: [Volatile<u32>; 2],
    __r7: Reserved<u32>,
    LEN: [Volatile<u32>; 2],
    __r8: Reserved<u32>,
    AREN: [Volatile<u32>; 2],
    __r9: Reserved<u32>,
    AFEN: [Volatile<u32>; 2],
    __r10: Reserved<u32>,
    PUD: Volatile<u32>,
    PUDCLK: [Volatile<u32>; 2],
}

/// GPIOピンの取りうるステート.
#[allow(unused_doc_comments)]
states! {
    Uninitialized, Input, Output, Alt
}

/// ステート`State`にあるGPIOピン.
///
/// `State` ジェネリックは常にインスタンス化できない型に対応し、
/// 与えられたGPIOピンのステートをマークして追跡するためだけに
/// 使用される。`Gpio` 構造体は `Uninitialized` 状態からスタートし、
/// 使用する前に `into_input`, `into_output`, `into_alt` の各メソッド
/// を使って `Input`, `Output`, `Alt` のいずれかに遷移する必要がある。
pub struct Gpio<State> {
    pin: u8,
    registers: &'static mut Registers,
    _state: PhantomData<State>,
}

impl<T> Gpio<T> {
    /// `self` をステート `S` に遷移させる. `self` を消費して
    /// ステート `S` の新しい `Gpio` インスタンスを返す。
    /// このメソッドは _絶対に_ 外に公開してはならない。
    #[inline(always)]
    fn transition<S>(self) -> Gpio<S> {
        Gpio {
            pin: self.pin,
            registers: self.registers,
            _state: PhantomData,
        }
    }
}

impl Gpio<Uninitialized> {
    /// ピン番号 `pin` 用の 新規 `GPIO` 構造体を返す.
    ///
    /// # Panics
    ///
    /// `pin` > `53` の場合はパニック.
    pub fn new(pin: u8) -> Gpio<Uninitialized> {
        if pin > 53 {
            panic!("Gpio::new(): pin {} exceeds maximum of 53", pin);
        }

        Gpio {
            registers: unsafe { &mut *(GPIO_BASE as *mut Registers) },
            pin: pin,
            _state: PhantomData,
        }
    }

    /// `self` の代替機能を `function` にする. selfを消費して
    /// ステート `ALT` の `Gpio` 構造体を返す.
    pub fn into_alt(self, function: Function) -> Gpio<Alt> {
        let g = self.pin as usize / 10;
        let r = self.pin as usize % 10;
        let f = function as u32;

        self.registers.FSEL[g].and_mask(!(0b111u32 << r * 3));
        self.registers.FSEL[g].or_mask(f << r * 3);
        self.transition()
    }

    /// このピンを _output_ ピンとして設定する. selfを消費して
    /// ステート `Output` の `Gpio` 構造体を返す.
    pub fn into_output(self) -> Gpio<Output> {
        self.into_alt(Function::Output).transition()
    }

    /// このピンを _input_ ピンとして設定する. selfを消費して
    /// ステート `Input` の `Gpio` 構造体を返す.
    pub fn into_input(self) -> Gpio<Input> {
        self.into_alt(Function::Input).transition()
    }
}

impl Gpio<Output> {
    /// ピンをセット（オンに）する.
    pub fn set(&mut self) {
        if self.pin < 32 {
            self.registers.SET[0].write(1 << self.pin);
        } else {
            self.registers.SET[1].write(1 << (self.pin - 32));
        }
    }

    /// ピンをクリア（オフに）する.
    pub fn clear(&mut self) {
        if self.pin < 32 {
            self.registers.CLR[0].write(1 << self.pin);
        } else {
            self.registers.CLR[1].write(1 << (self.pin - 32));
        }
    }
}

impl Gpio<Input> {
    /// ピンの値を読み取る. レベルがhighの場合は`true`,
    /// lowの場合は `false` を返す.
    pub fn level(&mut self) -> bool {
        if self.pin < 32 {
            self.registers.LEV[0].read() & (1 << self.pin) > 0
        } else {
            self.registers.LEV[1].read() & (1 << (self.pin - 32)) > 0
        }
    }
}
