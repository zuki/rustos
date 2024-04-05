#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

pub mod console;
pub mod mutex;
pub mod shell;

use console::kprintln;

// FIXME: You need to add dependencies here to
// test your drivers (Phase 2). Add them as needed.
use pi::timer;
use pi::gpio::Gpio;
use core::time::Duration;

unsafe fn kmain() -> ! {
    // FIXME: Start the shell.
    let mut leds = [
        Gpio::new(16).into_output(),
        Gpio::new(5).into_output()
    ];

    // FIXME: STEP 2: Continuously set and clear GPIO 16.
    loop {
        for i in 0..leds.len() {
            &leds[i].set();
            pi::timer::spin_sleep(Duration::from_millis(200));
            &leds[i].clear();
            pi::timer::spin_sleep(Duration::from_millis(200));
        }
    }
}
