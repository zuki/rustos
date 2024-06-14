use alloc::alloc::Global;
use alloc::boxed::Box;
use core::ops::Index;

use pi::interrupt::Interrupt;
use pi::local_interrupt::LocalInterrupt;

use crate::mutex::Mutex;
use crate::traps::TrapFrame;

// プログラマーガイド第10章
// AArch64 例外処理
/// 割り込みハンドラの型
pub type IrqHandler = Box<dyn FnMut(&mut TrapFrame) + Send>;
/// 割り込みハンドラをMutexで包んだ型
type IrqHandlerMutex = Mutex<Option<IrqHandler>>;
/// グローバル割り込みハンドラ型: 割り込みハンドラをMutexで包んだ型の配列
type GlobalIrqHandlers = [IrqHandlerMutex; Interrupt::MAX];
/// ローカル割り込みハンドラ型: 割り込みハンドラをMutexで包んだ型の配列
type LocalIrqHandlers = [IrqHandlerMutex; LocalInterrupt::MAX];

/// グローバルIRQハンドラレジストリ.
// GlobalIrq.0 = GlobalIrqHandlers
pub struct GlobalIrq(GlobalIrqHandlers);
/// ローカル（per-cpu）IRQハンドラレジストリ. (QA7: Chapter 4)
pub struct LocalIrq(LocalIrqHandlers);
/// グローバルRIQハンドラレジストリ.
/// このカーネルはFIQ割り込みを1つしかサポートしない.
pub struct Fiq(IrqHandlerMutex);

impl GlobalIrq {
    pub const fn new() -> GlobalIrq {
        GlobalIrq([
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
        ])
    }
}

impl LocalIrq {
    pub const fn new() -> LocalIrq {
        LocalIrq([
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
            Mutex::new(None),
        ])
    }
}

impl Fiq {
    pub const fn new() -> Fiq {
        Fiq(Mutex::new(None))
    }
}

impl Index<Interrupt> for GlobalIrq {
    type Output = IrqHandlerMutex;

    /// GlobalIrqから指定の割り込みenumのハンドラを返す
    fn index(&self, int: Interrupt) -> &IrqHandlerMutex {
        use Interrupt::*;
        let index = match int {
            Timer1 => 0,
            Timer3 => 1,
            Usb => 2,
            Gpio0 => 3,
            Gpio1 => 4,
            Gpio2 => 5,
            Gpio3 => 6,
            Uart => 7,
        };
        &self.0[index]
    }
}

impl Index<LocalInterrupt> for LocalIrq {
    type Output = IrqHandlerMutex;

    fn index(&self, int: LocalInterrupt) -> &IrqHandlerMutex {
        // Lab 5 1.C
        use LocalInterrupt::*;
        let index = match int {
            CNTPSIRQ => 0,
            CNTPNSIRQ => 1,
            CNTHPIRQ => 2,
            CNTVIRQ => 3,
            MAILBOX0 => 4,
            MAILBOX1 => 5,
            MAILBOX2 => 6,
            MAILBOX3 => 7,
            GPU => 8,
            PMU => 9,
            AXIOUTSTNADING => 10,
            LOCALTIMER => 11,
        };
        &self.0[index]
    }
}

impl Index<()> for Fiq {
    type Output = IrqHandlerMutex;

    fn index(&self, _: ()) -> &IrqHandlerMutex {
        // Lab 5 2.B
        &self.0
    }
}

/// IRQ (とFIQ）ハンドラレジストリの振る舞いを定義するトレイト.
pub trait IrqHandlerRegistry<I> {
    fn register(&self, int: I, handler: IrqHandler);
    fn invoke(&self, int: I, tf: &mut TrapFrame);
}

/// `IrqHandlerMutex` を返すすべてのインデックス可能な構造体に対する
/// `IrqHandlerRegistry` トレイトの包括的な実装.
impl<I, T> IrqHandlerRegistry<I> for T
where
    T: Index<I, Output = IrqHandlerMutex>,
{
    /// 割り込みに対するirqㇵンドラを登録する.
    fn register(&self, int: I, handler: IrqHandler) {
        *(self.index(int).lock()) = Some(handler);
    }

    /// 指定された割り込みに対するirQハンドラを実行する.
    fn invoke(&self, int: I, tf: &mut TrapFrame) {
        match &mut (*self.index(int).lock()) {
            Some(handler) => handler(tf),
            None => panic!("invoke invalid interrupt"),
        }
    }
}
