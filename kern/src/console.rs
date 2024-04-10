use core::fmt;
use pi::uart::MiniUart;
use shim::io;

use crate::mutex::Mutex;

/// コンソールの読み書きを可能にするグローバルなシングルトン.
pub struct Console {
    inner: Option<MiniUart>,
}

impl Console {
    /// `Console`の新規インスタンスを作成する.
    const fn new() -> Console {
        Console { inner: None }
    }

    /// まだ初期化されていなければコンソールを初期化する.
    #[inline]
    fn initialize(&mut self) {
        if self.inner.is_none() {
            self.inner = Some(MiniUart::new());
        }
    }

    /// 必要なら初期化した内部の`MiniUart`への可変借用を返す.
    fn inner(&mut self) -> &mut MiniUart {
        self.initialize();
        self.inner.as_mut().unwrap()
    }

    /// UARTデバイスから1バイト読み込む。1バイト読み込めるまでブロックする.
    pub fn read_byte(&mut self) -> u8 {
        self.inner().read_byte()
    }

    /// バイト `byte` をUARTデバイスに書き込む.
    pub fn write_byte(&mut self, byte: u8) {
        self.inner().write_byte(byte);
    }
}

impl io::Read for Console {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner().read(buf)
    }
}

impl io::Write for Console {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.inner().write_str(s)
    }
}

/// グローバルな `Console` シングルトン.
pub static CONSOLE: Mutex<Console> = Mutex::new(Console::new());

/// `kprint[ln]!` マクロから呼び出される内部関数.
#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    #[cfg(not(test))]
    {
        use core::fmt::Write;
        let mut console = CONSOLE.lock();
        console.write_fmt(args).unwrap();
    }

    #[cfg(test)]
    {
        print!("{}", args);
    }
}

/// カーネル空間用の `println!`.
pub macro kprintln {
    () => (kprint!("\n")),
    ($fmt:expr) => (kprint!(concat!($fmt, "\n"))),
    ($fmt:expr, $($arg:tt)*) => (kprint!(concat!($fmt, "\n"), $($arg)*))
}

/// カーネル空間用の `print!`,.
pub macro kprint($($arg:tt)*) {
    _print(format_args!($($arg)*))
}
