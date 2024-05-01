#![feature(alloc_error_handler)]
#![feature(const_fn)]
#![feature(decl_macro)]
#![feature(asm)]
#![feature(global_asm)]
#![feature(optin_builtin_traits)]
#![feature(raw_vec_internals)]
#![feature(panic_info_message)]
#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
mod init;

extern crate alloc;

pub mod allocator;
pub mod console;
pub mod fs;
pub mod mutex;
pub mod shell;

use console::{CONSOLE, kprint, kprintln};

use allocator::Allocator;
use fs::FileSystem;
use alloc::vec::Vec;
use fs::sd::Sd;

use fat32::traits::BlockDevice;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();

fn kmain() -> ! {
    unsafe {
        ALLOCATOR.initialize();
        //FILESYSTEM.initialize();
    }

    kprintln!("Welcome to cs3210!");

    match unsafe { Sd::new() } {
        Ok(mut sd) => {
            let mut buf = [0_u8; 512];
            match &sd.read_sector(0, &mut buf) {
                Ok(n) => kprintln!("read {} bytes", n),
                Err(e) => kprintln!("error in read: {:?}", e),
            }
        }
        Err(e) => kprintln!("error in new: {:?}", e),
    }


/*
    let mut v = Vec::new();
    for i in 0..30 {
        v.push(i);
        kprintln!("{:?}", v);
    }


    let mut atags = atags::Atags::get();
    while let Some(atag) = atags.next() {
        match atag.cmd() {
            Some(cmd) => {
                kprintln!("Cmd(");
                for item in cmd.split_whitespace() {
                    kprintln!("  {}", item);
                }
                kprintln!(")");
            }
            _ => kprintln!("{:#?}", atag),
        }
    }
*/
    shell::shell("> ");
    loop {}
}
