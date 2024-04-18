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
use pi::atags;

#[cfg_attr(not(test), global_allocator)]
pub static ALLOCATOR: Allocator = Allocator::uninitialized();
pub static FILESYSTEM: FileSystem = FileSystem::uninitialized();

fn kmain() -> ! {
/*
    unsafe {
        ALLOCATOR.initialize();
        FILESYSTEM.initialize();
    }
*/
    kprintln!("Welcome to cs3210!");

    let mut atags = atags::Atags::get();
    loop {
        match atags.next() {
            Some(atag) => {
                match atag.cmd() {
                    Some(cmd) => {
                        kprintln!("Cmd(");
                        let mut iter = cmd.split_whitespace();
                        loop {
                            match iter.next() {
                                Some(item) => kprintln!("  {}", item),
                                None => break,
                            }
                        }
                        kprintln!(")");
                    }
                    _ => kprintln!("{:#?}", atag),
                }
            }
            None => break,
        }
    }
//    shell::shell("> ");
    loop {}
}
