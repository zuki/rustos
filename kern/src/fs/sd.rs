use core::time::Duration;
use shim::io;
use shim::ioerr;
use pi::timer;

use fat32::traits::BlockDevice;

extern "C" {
    /// 最後に発生したSDコントローラのエラーを表すグローバル.
    static sd_err: i64;

    /// SDカードコントローラを初期化する.
    ///
    /// 初期化に成功した場合は0を返す。初期化に失敗した場合、
    /// タイムアウトが発生した場合は-1、SDコントローラへの
    /// コマンド送信エラーが発生した場合は-2を返す。
    fn sd_init() -> i32;

    /// SDカードからセクタ `n` (512 バイト) を読み込み、`buffer` に
    /// 書き込む。`buffer` が少なくとも512バイトのメモリを指していない
    /// と未定義の動作となる。また、この関数のcallerは `buffer` が
    /// 少なくとも4バイトアラインされていることを確認する必要がある。
    ///
    /// 成功死た場合は読み込んだバイト数（正数）を返す。
    ///
    /// エラーの場合は0を返す。真のエラーコードはグローバル変数
    /// `sd_err` に格納される。`sd_err` には、タイムアウトが発生した
    /// 場合は -1、SDコントローラへのコマンド送信エラーが発生した
    /// 場合は -2がセットされる。その他のエラーコードも可能であるが
    /// 0より小さい値としてのみ定義される。
    fn sd_readsector(n: i32, buffer: *mut u8) -> i32;
}

// FIXME: Define a `#[no_mangle]` `wait_micros` function for use by `libsd`.
// The `wait_micros` C signature is: `void wait_micros(unsigned int);`
#[no_mangle]
fn wait_micros(micros: u32) {
    timer::spin_sleep(Duration::from_micros(micros as u64))
}

/// SDカードコントローラへのハンドル.
#[derive(Debug)]
pub struct Sd();

impl Sd {
    /// SDカードコントローラを初期化し、そのハンドルを返す。
    /// callerはこのメソッドがカーネルの初期化中に一度だけ
    /// 呼び出されることを保証しなければならない。アトミックな
    /// メモリアクセスを使えば安全なRustコードでこの要件を実現
    /// できるがまだメモリ管理ユニット（MMU）を書いていないので
    /// 使えない。
    pub unsafe fn new() -> Result<Sd, io::Error> {
        match sd_init() {
            0 => Ok(Sd{}),
            -1 => ioerr!(TimedOut, "Timeout occured in sd_init()"),
            -2 => ioerr!(BrokenPipe, "could not send init command"),
            _ => ioerr!(Other, "unkown initialization error pccured"),
        }
    }
}


impl BlockDevice for Sd {
    /// SDカードからセクタ `n` を `buf` に読み込む。
    /// 成功した場合はバイト数が返される。
    ///
    /// # エラー
    ///
    /// `buf.len() < 512` または  `n > 2^31 - 1` (`i32` の最大値)の
    /// 場合はエラー種別 `InvalidInput` のI/Oエラーが返される。
    ///
    /// SDカードからの読み込み中にタイムアウトが発生した場合は
    ///  `TimedOut` のエラーが返される。
    ///
    /// その他のエラーに対してはエラー種別 `Other` のエラーが返される。
    fn read_sector(&mut self, n: u64, buf: &mut [u8]) -> io::Result<usize> {
        if buf.len() < 512 {
            return ioerr!(InvalidInput, "invalid buf len");
        }

        if n > i32::max_value() as u64 {
            return ioerr!(InvalidInput, "invalid sector number");
        }

        let result = unsafe { sd_readsector(n as i32, buf.as_mut_ptr()) };

        if result == 0 {
            return match unsafe { sd_err } {
                -1 => ioerr!(TimedOut, "sd read timed out"),
                -2 => ioerr!(BrokenPipe, "could not send command"),
                _ => ioerr!(Other, "unknown read error")
            }
        }

        Ok(512)

    }

    fn write_sector(&mut self, _n: u64, _buf: &[u8]) -> io::Result<usize> {
        unimplemented!("SD card and file system are read only")
    }
}
