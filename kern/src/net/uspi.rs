#![allow(non_snake_case)]

use alloc::boxed::Box;
//use alloc::string::String;
use core::slice::from_ref;
use core::str::from_utf8;
use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;
use core::time::Duration;

use pi::interrupt::{Controller, Interrupt};
use pi::timer::spin_sleep;
use smoltcp::wire::EthernetAddress;

use crate::mutex::Mutex;
use crate::net::Frame;
use crate::traps::irq::IrqHandlerRegistry;
use crate::{ALLOCATOR, GLOBAL_IRQ, FIQ};

const DEBUG_USPI: bool = false;
pub macro uspi_trace {
    () => (if DEBUG_USPI { trace!("\n") } ),
    ($fmt:expr) => (if DEBUG_USPI { trace!(concat!($fmt, "\n")) }),
    ($fmt:expr, $($arg:tt)*) => (if DEBUG_USPI { trace!(concat!($fmt, "\n"), $($arg)*) })
}

pub type TKernelTimerHandle = u64;
pub type TKernelTimerHandler = Option<
    unsafe extern "C" fn(hTimer: TKernelTimerHandle, pParam: *mut c_void, pContext: *mut c_void),
>;
pub type TInterruptHandler = Option<unsafe extern "C" fn(pParam: *mut c_void)>;

mod inner {
    use core::convert::TryInto;
    use core::ptr;
    use core::time::Duration;

    use super::{TKernelTimerHandle, TKernelTimerHandler};
    use crate::net::Frame;
    use crate::param::USPI_TIMER_HZ;

    #[allow(non_camel_case_types)]
    type c_uint = usize;

    pub struct USPi(());

    extern "C" {
        /// 失敗時に0を返す
        fn USPiInitialize() -> i32;
        /// ethernetコントローラが利用可能化チェックする.
        /// 利用可能な場合は != 0 を返す
        fn USPiEthernetAvailable() -> i32;
        fn USPiGetMACAddress(Buffer: &mut [u8; 6]);
        /// リンクが上がっている場合  != 0 を返す
        fn USPiEthernetIsLinkUp() -> i32;
        /// 失敗時に0を返す
        fn USPiSendFrame(pBuffer: *const u8, nLength: u32) -> i32;
        /// pBufferのサイズはUSPI_FRAME_BUFFER_SIZEデなければならない
        /// 利用可能なフレームがない、または失敗の場合は 0を返す
        fn USPiReceiveFrame(pBuffer: *mut u8, pResultLength: *mut u32) -> i32;
        /// タイマーハンドルを返す（失敗時は0を返す）
        fn TimerStartKernelTimer(
            pThis: TKernelTimerHandle,
            nDelay: c_uint, // HZ 単位
            pHandler: TKernelTimerHandler,
            pParam: *mut core::ffi::c_void,
            pContext: *mut core::ffi::c_void,
        ) -> c_uint;
        fn TimerGet() -> TKernelTimerHandle;
    }

    impl !Sync for USPi {}

    impl USPi {
        /// callerはこの関数をカーネルの生存期間に1度だけ
        /// 呼び出す必要がある
        pub unsafe fn initialize() -> Self {
            assert!(USPiInitialize() != 0);
            USPi(())
        }

        /// RPiでethernetが利用可能であるか否かを返す
        pub fn is_eth_available(&mut self) -> bool {
            unsafe { USPiEthernetAvailable() != 0 }
        }

        /// RPiのMACアドレスを返す
        pub fn get_mac_address(&mut self, buf: &mut [u8; 6]) {
            unsafe { USPiGetMACAddress(buf) }
        }

        /// RPiでethernetリンクが上がっているか否かをチェックする
        pub fn is_eth_link_up(&mut self) -> bool {
            unsafe { USPiEthernetIsLinkUp() != 0 }
        }

        /// USPiSendFrameを使ってethernetフレームを送信する
        pub fn send_frame(&mut self, frame: &Frame) -> Option<i32> {
            trace!("Send frame {:?}", frame);
            let result = unsafe { USPiSendFrame(frame.as_ptr(), frame.len()) };
            match result {
                0 => None,
                n => Some(n),
            }
        }

        /// USPiRecvFrameを使ってethernetフレームを受診する
        pub fn recv_frame<'a>(&mut self, frame: &mut Frame) -> Option<i32> {
            let mut result_len = 0;
            trace!("Recv frame {:?}", frame);
            let result = unsafe { USPiReceiveFrame(frame.as_mut_ptr(), &mut result_len) };
            frame.set_len(result_len);
            match result {
                0 => None,
                n => Some(n),
            }
        }

        /// `TimerStartKernelHandler`のラッパー関数
        pub fn start_kernel_timer(&mut self, delay: Duration, handler: TKernelTimerHandler) {
            trace!(
                "Core {}, delay {:?}, handler {:?}",
                aarch64::affinity(),
                &delay,
                handler.map(|v| v as usize as *mut u8)
            );

            let divisor = (1000 / USPI_TIMER_HZ) as u128;
            let delay_as_hz = (delay.as_millis() + divisor - 1) / divisor;

            if let Ok(c_delay) = delay_as_hz.try_into() {
                unsafe {
                    TimerStartKernelTimer(
                        TimerGet(),
                        c_delay,
                        handler,
                        ptr::null_mut(),
                        ptr::null_mut(),
                    );
                }
            }
        }
    }
}

pub use inner::USPi;

#[no_mangle]
fn malloc(size: u32) -> *mut c_void {
    // Lab 5 2.B
    let alloc_size = size as usize + core::mem::size_of::<Layout>();
    unsafe {
        let layout = Layout::from_size_align_unchecked(alloc_size, 16);
        let ptr = ALLOCATOR.alloc(layout) as *mut Layout;
        ptr.write(layout);
        ptr.add(1) as *mut c_void
    }
}

#[no_mangle]
fn free(ptr: *mut c_void) {
    // Lab 5 2.B
    unsafe {
        let layout_ptr = (ptr as *mut Layout).sub(1);
        let layout = layout_ptr.read();
        ALLOCATOR.dealloc(layout_ptr as *mut u8, layout);
    }
}

#[no_mangle]
pub fn TimerSimpleMsDelay(nMilliSeconds: u32) {
    // Lab 5 2.B
    spin_sleep(Duration::from_millis(nMilliSeconds as u64));
}

#[no_mangle]
pub fn TimerSimpleusDelay(nMicroSeconds: u32) {
    // Lab 5 2.B
    spin_sleep(Duration::from_micros(nMicroSeconds as u64));
}

#[no_mangle]
pub fn MsDelay(nMilliSeconds: u32) {
    // Lab 5 2.B
    spin_sleep(Duration::from_millis(nMilliSeconds as u64));
}

#[no_mangle]
pub fn usDelay(nMicroSeconds: u32) {
    // Lab 5 2.B
    spin_sleep(Duration::from_micros(nMicroSeconds as u64));
}

/// `pHandler`をカーネルのIRQハンドラレジストリに登録する.
/// 次回カーネルが`nIRQ`シグナルを受診した際、ハンドラ関数
/// `pHandler`が`pPrama`で実行される。
///
/// `nIRQ == Interrupt::Usb`の場合はFIQ割り込みハンドラレジスタに
/// 登録する。それ以外の場合は、グローバルIRQ割り込みハンドラに
/// ハンドラを登録する。
#[no_mangle]
pub unsafe fn ConnectInterrupt(nIRQ: u32, pHandler: TInterruptHandler, pParam: *mut c_void) {
    // Lab 5 2.B
    assert!(nIRQ != Interrupt::Timer3 as u32 && nIRQ != Interrupt::Usb as u32, "invalide nRIQ");
    assert!(pHandler.is_some(), "pHandler is None");

    let handler = pHandler.unwrap();

    match Interrupt::from(nIRQ as usize) {
        Interrupt::Usb => {
            let mut controller = Controller::new();
            controller.enable_fiq(Interrupt::Usb);
            FIQ.register((), Box::new(|tf| { handler(pParam) }));
        }
        Interrupt::Timer3 => {
            let mut controller = Controller::new();
            controller.enable(Interrupt::Timer3);
            GLOBAL_IRQ.register(Interrupt::Timer3, Box::new(|tf| { handler(pParam) }));
        }
        _ => {}
    }
}

/// `uspi_trace!`マクロを使ってUSPiからのログメッセージを書き出す.
#[no_mangle]
pub unsafe fn DoLogWrite(_pSource: *const u8, _Severity: u32, pMessage: *const u8) {
    // Lab 5 2.B
    let msg_str: &[u8] = from_ref(&*pMessage);
    let message: &str = match from_utf8(msg_str) {
        Ok(s) => s,
        Err(_) => "convert error",
    };
    uspi_trace!("{}", message);

}

#[no_mangle]
pub fn DebugHexdump(_pBuffer: *const c_void, _nBufLen: u32, _pSource: *const u8) {
    unimplemented!("You don't have to implement this")
}

#[no_mangle]
pub unsafe fn uspi_assertion_failed(pExpr: *const u8, pFile: *const u8, nLine: u32) {
    // Lab 5 2.B
    let expr_str: &[u8] = from_ref(&*pExpr);
    let expression: &str = match from_utf8(expr_str) {
        Ok(s) => s,
        Err(_) => "expression",
    };
    let file_str: &[u8] = from_ref(&*pFile);
    let file: &str = match from_utf8(file_str) {
        Ok(s) => s,
        Err(_) => "Unknown file",
    };
    uspi_trace!("{} [{}]: assert failed: {}", file, nLine, expression);
}

pub struct Usb(pub Mutex<Option<USPi>>);

impl Usb {
    pub const fn uninitialized() -> Usb {
        Usb(Mutex::new(None))
    }

    pub fn initialize(&self) {
        let mut inner = self.0.lock();
        if let None = *inner {
            *inner = Some(unsafe { USPi::initialize() });
        }
    }

    pub fn is_eth_available(&self) -> bool {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .is_eth_available()
    }

    pub fn get_eth_addr(&self) -> EthernetAddress {
        let mut buf = [0; 6];
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .get_mac_address(&mut buf);
        return EthernetAddress::from_bytes(&buf);
    }

    pub fn is_eth_link_up(&self) -> bool {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .is_eth_link_up()
    }

    pub fn send_frame(&self, frame: &Frame) -> Option<i32> {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .send_frame(frame)
    }

    pub fn recv_frame(&self, frame: &mut Frame) -> Option<i32> {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .recv_frame(frame)
    }

    pub fn start_kernel_timer(&self, delay: Duration, handler: TKernelTimerHandler) {
        self.0
            .lock()
            .as_mut()
            .expect("USB not initialized")
            .start_kernel_timer(delay, handler)
    }
}
