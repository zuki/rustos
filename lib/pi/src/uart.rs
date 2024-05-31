use core::fmt;
use core::time::Duration;

//use shim::const_assert_size;
use shim::io;

use volatile::prelude::*;
use volatile::{ReadVolatile, Volatile};

use crate::common::IO_BASE;
use crate::gpio::{Function, Gpio};
use crate::timer;

/// `MU` レジスタの基底アドレス.
const MU_REG_BASE: usize = IO_BASE + 0x215040;

/// `AUXENB` レジスタ: BCM2837ドキュメントのページ9より.
const AUX_ENABLES: *mut Volatile<u8> = (IO_BASE + 0x215004) as *mut Volatile<u8>;

/// `AUX_MU_LSR_REG` レジスタのビットフィールドを表すEnum.
#[repr(u8)]
enum LsrStatus {
    DataReady = 1,
    TxAvailable = 1 << 5,
}

#[repr(C)]
#[allow(non_snake_case)]
struct Registers {
    // FIXME: Declare the "MU" registers from page 8.
    MU_IO: Volatile<u32>,
    MU_IER: Volatile<u32>,
    MU_IIR: Volatile<u32>,
    MU_LCR: Volatile<u32>,
    MU_MCR: Volatile<u32>,
    MU_LSR: ReadVolatile<u32>,
    MU_MSR: ReadVolatile<u32>,
    MU_SCRATCH: Volatile<u32>,
    MU_CNT: Volatile<u32>,
    MU_STAT: ReadVolatile<u32>,
    MU_BAUD: Volatile<u32>,
}

/// Raspberry Piの "mini UART".
pub struct MiniUart {
    registers: &'static mut Registers,
    timeout: Option<Duration>,
}

impl MiniUart {
    /// mini UARTを初期化する。それには補助ペリフェラルとして
    /// 有効にし、データサイズを8 ビット、BAUDレートを ~115200
    /// （270 分周）、GPIOピン14と15を代替機能5（TXD1/RDXD1）に
    /// それぞれ設定し、最後にUARTトランスミッタとレシーバを
    /// 有効にする。
    ///
    /// デフォルトでは読み出しはタイムアウトしない。読み取り
    /// タイムアウトを設定するには`set_read_timeout()`を使用する。
    pub fn new() -> MiniUart {
        let registers = unsafe {
            // mini UARTを補助デバイスとして有効にする.
            (*AUX_ENABLES).or_mask(1);
            &mut *(MU_REG_BASE as *mut Registers)
        };

        // FIXME: Implement remaining mini UART initialization.
        &registers.MU_CNT.write(0);
        &registers.MU_LCR.write(3);
        &registers.MU_BAUD.write(270);
        Gpio::new(14).into_alt(Function::Alt5);
        Gpio::new(15).into_alt(Function::Alt5);
        &registers.MU_CNT.write(3);

        MiniUart {
            registers,
            timeout: None,
        }
    }

    /// readタイムアウトをDureation `t` に設定する.
    pub fn set_read_timeout(&mut self, t: Duration) {
        self.timeout = Some(t);
    }

    /// バイト `byte` を書き出す. 出力FIFOに空きができるmで
    /// このメソッドはブロックする.
    pub fn write_byte(&mut self, byte: u8) {
        while self.registers.MU_LSR.read() & LsrStatus::TxAvailable as u32 == 0 {}
        self.registers.MU_IO.write(byte as u32);
    }

    /// 少なくとも読み込めるバイトが1つある場合は `true` を返す.
    /// このメソッドが `true` を返した場合は続けて `read_byte` を
    /// 呼び出すと即座に `read_byte` が返ることが保証される。
    /// このメソッドはブロックしない。
    pub fn has_byte(&self) -> bool {
        (self.registers.MU_LSR.read() as u8) & LsrStatus::DataReady as u8 != 0
    }

    /// 読み取り可能なバイトができるまでブロックする。readタイムアウトが
    /// 設定されている場合、このメソッドは最大でその時間ブロックする。
    /// そうでない場合、このメソッドは読み取り可能なバイトができるまで
    /// 無限にブロックする。
    ///
    /// 読み取り可能なバイトができたら `Ok(())` を返す。読み取り可能な
    /// バイトの大気中にタイムアウトした場合は `Err(())` を返す。
    /// このメソッドが `Ok(())` を返した場合は続けて `read_byte` を
    /// 呼び出すと即座に `read_byte` が返ることが保証される。
    pub fn wait_for_byte(&self) -> Result<(), ()> {
        match self.timeout {
            Some(timeout) => {
                let end = timer::current_time() + timeout;
                while !self.has_byte() && timer::current_time() < end {}

                if timer::current_time() < end {
                    Ok(())
                } else {
                    Err(())
                }
            }
            None => {
                while !self.has_byte() {}
                Ok(())
            }
        }
    }

    /// 1バイト読み込む。バイトが読み込み可能になるまでブロックする.
    pub fn read_byte(&self) -> u8 {
        while !self.has_byte() {}
        self.registers.MU_IO.read() as u8
    }
}

// FIXME: MiniUart`に対して`fmt::Write`を実装する。b'\n' バイトを
// 書き込む前に b'\r' バイトを書き込む。
impl fmt::Write for MiniUart {
    fn write_str(&mut self, s: &str) -> Result<(), core::fmt::Error> {
        for byte in s.as_bytes().iter() {
            if *byte == b'\n' {
                self.write_byte(b'\r');
            }
            self.write_byte(*byte);
        }
        Ok(())
    }
}

mod uart_io {
    use super::io;
    use super::MiniUart;
    use shim::ioerr;

    // FIXME: `MiniUart`に対して `io::Read` と `io::Write` を実装する.
    //
    // `io::Read::read()`の実装はreadタイムアウトを尊重する必要があり、
    // _最初の_ バイトが読み込み可能になるまで最大タイムアウト待つ必要が
    // ある。それ以降のバイトは待つ必要はないが可能な限り多くのバイトを
    // 読み込む必要がある。読み込みがタイムアウトした場合は`TimedOut`と
    // いうエラーを返す必要がある。
    //
    // `io::Write::write()`メソッドはリターンする前に要求されたバイトを
    // すべて書き込む必要がある。
    impl io::Read for MiniUart {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            match self.wait_for_byte() {
                Ok(_) => {
                    for i in 0..buf.len() {
                        if !self.has_byte() {
                            return Ok(i);
                        }
                        buf[i] = self.read_byte();
                    }
                    Ok(buf.len())
                }
                Err(_) => ioerr!(TimedOut, "timedout"),
            }
        }
    }

    impl io::Write for MiniUart {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            for byte in buf.iter() {
                self.write_byte(*byte);
            }
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}
