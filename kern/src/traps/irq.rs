use alloc::boxed::Box;
use pi::interrupt::Interrupt;

use crate::mutex::Mutex;
use crate::traps::TrapFrame;

pub type IrqHandler = Box<dyn FnMut(&mut TrapFrame) + Send>;
pub type IrqHandlers = [Option<IrqHandler>; Interrupt::MAX];

pub struct Irq(Mutex<Option<IrqHandlers>>);

impl Irq {
    pub const fn uninitialized() -> Irq {
        Irq(Mutex::new(None))
    }

    pub fn initialize(&self) {
        *self.0.lock() = Some([None, None, None, None, None, None, None, None]);
    }

    /// 割り込み `int` 用ののirqハンドラを登録する.
    /// callerはこの関数を呼び出す前に `initialize()` を呼び出しておく必要がある。
    pub fn register(&self, int: Interrupt, handler: IrqHandler) {
        if let Some(handlers) = self.0.lock().as_mut() {
            handlers[Interrupt::to_index(int)] = Some(handler);
        }
    }

    /// 指定された割り込みのirqハンドラを実行する.
    /// callerはこの関数を呼び出す前に `initialize()` を呼び出しておく必要がある。
    pub fn invoke(&self, int: Interrupt, tf: &mut TrapFrame) {
        if let Some(handlers) = self.0.lock().as_mut() {
            let handler = &mut handlers[Interrupt::to_index(int)];
            handler.as_mut().unwrap()(tf);
        }
    }
}
