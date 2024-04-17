#![feature(asm)]
#![feature(global_asm)]

#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

use xmodem::Xmodem;
use core::time::Duration;
use pi::uart;
use core::fmt::Write;

/// ロードするバイナリとブートローダの開始アドレスr.
const BINARY_START_ADDR: usize = 0x80000;
const BOOTLOADER_START_ADDR: usize = 0x4000000;

/// ロードされるバイナリがロードを期待する場所へのポインタ.
const BINARY_START: *mut u8 = BINARY_START_ADDR as *mut u8;

/// ブートローダとロードされるバイナリの開始アドレスの間の空きスペース.
const MAX_BINARY_SIZE: usize = BOOTLOADER_START_ADDR - BINARY_START_ADDR;

/// 無条件にアドレス`addr`に分岐する.
unsafe fn jump_to(addr: *mut u8) -> ! {
    asm!("br $0" : : "r"(addr as usize));
    loop {
        asm!("wfe" :::: "volatile")
    }
}

fn kmain() -> ! {
    // FIXME: Implement the bootloader.
    let mut uart = uart::MiniUart::new();
    &uart.set_read_timeout(Duration::from_millis(750));
    loop {
        match Xmodem::receive(&mut uart, unsafe { core::slice::from_raw_parts_mut(BINARY_START, MAX_BINARY_SIZE) }) {
            Ok(_) => {
                &uart.write_str("run kernel\n");
                unsafe {jump_to(BINARY_START_ADDR as *mut u8); }
            }
            Err(e) => {
                &uart.write_str(e.get_ref().unwrap());
                &uart.write_str("\n");
                continue;
            }
        }
    }
}
