mod frame;
mod syndrome;
mod syscall;

pub mod irq;
pub use self::frame::TrapFrame;

use pi::interrupt::{Controller, Interrupt};

use self::syndrome::Syndrome;
use self::syscall::handle_syscall;
use crate::console::{CONSOLE, kprint, kprintln};
use crate::IRQ;
use aarch64::*;

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Kind {
    Synchronous = 0,
    Irq = 1,
    Fiq = 2,
    SError = 3,
}

#[repr(u16)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Source {
    CurrentSpEl0 = 0,
    CurrentSpElx = 1,
    LowerAArch64 = 2,
    LowerAArch32 = 3,
}

#[repr(C)]
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub struct Info {
    source: Source,
    kind: Kind,
}

/// この関数は例外が発生した際に呼び出される。引数`info`は
/// 発生した例外のソースと種類を示す。`esr`は例外シンドローム
/// レジスタの値、`tf`は例外のトラップフレームへのポインタである。
#[no_mangle]
pub extern "C" fn handle_exception(info: Info, esr: u32, tf: &mut TrapFrame, far: u64) {
    //kprintln!("info: {:?}, esr: 0x{:x}", info, esr);
    match info.kind {
        Kind::Synchronous => {
            match Syndrome::from(esr) {
                Syndrome::Brk(n) => {
                    // kprintln!("Syndrome::Brk({})", n);
                    // kprintln!("  ELR: 0x{:x}", tf.elr);
                    crate::shell::shell("debug > ");
                    tf.elr += 4;
                }
                Syndrome::Svc(n) => {
                    // kprintln!("Syndrome::Svc({})", n);
                    handle_syscall(n as u16, tf);
                }
                s => panic!("Unexpected syndrome: {:?}\ninfo: {:x?}\nesr : 0x{:08X}\nfar : 0x{:016X}\ntf:\n{:?}", s, info, esr, far, tf),
            }
        }
        Kind::Irq => {
            let controller = Controller::new();
            for int in Interrupt::iter() {
                if controller.is_pending(*int) {
                    //kprintln!("IRQ: {:?}", *int as u32);
                    IRQ.invoke(*int, tf);
                }
            }
        }
        _ => {
            //
        }
    }
}
