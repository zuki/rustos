#![feature(asm)]
#![feature(never_type)]
#![no_std]
#![no_main]

mod cr0;

use core::time::Duration;

use kernel_api::syscall::*;
use kernel_api::{print, println, OsResult};

fn main() {
    let result = main_inner();
    if let Err(error) = result {
        println!("Terminating with error: {:?}", error);
    }
}

fn main_inner() -> OsResult<!> {
    // Lab 5 3
    let descriptor = sock_create();
    sock_listen(descriptor, 80_u16);
    print!("connect ");
    loop {
        let status = sock_status(descriptor)?;
        if status.can_send {
            println!(" ok.");
            break;
        } else {
            print!(".");
            sleep(Duration::from_secs(1));
        }
    }
    let welcome: &str = "welcome echo server!\n";
    sock_send(descriptor, welcome.as_bytes());

    let mut buf = [0_u8; 512];
    loop {
        let size = sock_recv(descriptor, &mut buf)?;
        println!("{:?}", &buf[..size]);
        let _ = sock_send(descriptor, &buf[..size])?;
    }
}
