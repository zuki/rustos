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
    let mut buf = [0_u8; 512];

    println!("[ECHO] sock_create");
    let descriptor = sock_create();
    println!("[ECHO] socket {} created", descriptor.raw());

    println!("[ECHO] sock_listen");
    sock_listen(descriptor, 80_u16);
    println!("[ECHO] listen socket {} port 80", descriptor.raw());

    println!("[ECHO] sock_status 0");
    let status = sock_status(descriptor)?;
    println!("[ECHO] 0 {:?}", status);

    //let mut active = false;
    loop {
        println!("[ECHO] sock_status 1");
        let status = sock_status(descriptor)?;
        println!("[ECHO] 1 {:?}", status);
        if status.can_send {
            println!("connected");
            break;
        } else {
            println!("Waiting client connection...");
            sleep(Duration::from_secs(1));
        }
    /*
        if status.is_active && !active {
            println!("connected");
            break;
        } else if !status.is_active && active {
            println!("disconnected");
        } else {
            println!("Waiting client connection...");
            sleep(Duration::from_secs(1));
        }
        active = status.is_active;
    */
    }

    println!("[ECHO] sock_status 2");
    let status = sock_status(descriptor)?;
    println!("[ECHO] 2 {:?}", status);
    if  status.can_send {
        let mes = "Welcome to RPi echo server!";
        println!("[ECHO] sock_send 1");
        sock_send(descriptor, mes.as_bytes());
    }

    println!("[ECHO] sock_status 3");
    let status = sock_status(descriptor)?;
    println!("[ECHO] 3 {:?}", status);

    loop {
        println!("[ECHO] sock_status 4");
        let status = sock_status(descriptor)?;
        if status.can_recv {
            println!("[ECHO] sock_recv 1");
            let size = sock_recv(descriptor, &mut buf)?;
            if size > 0 {
                println!("{:?}", &buf[..size]);
                println!("[ECHO] sock_status 5");
                let status = sock_status(descriptor)?;
                if status.can_send {
                    println!("[ECHO] sock_send 2");
                    let _ = sock_send(descriptor, &buf[..size])?;
                }
            }
        }
    }
}
