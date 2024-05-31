#![feature(asm)]
#![no_std]
#![no_main]

mod cr0;

use kernel_api::println;
use kernel_api::syscall::{getpid, time};

fn fib(n: u64) -> u64 {
    match n {
        0 => 1,
        1 => 1,
        n => fib(n - 1) + fib(n - 2),
    }
}

fn main() {
    let pid = getpid();
    let beg = time();
    //println!("PID [{}] fib started: {:?}", pid, beg);

    let rtn = fib(20);

    let end = time();
    println!("PID [{}] fib(20) = {} ({:?})", pid, rtn, end - beg);
    //println!("PID [{}] fib ended: {:?}", pid, end);
}
