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
    let started = time();
    let pid = getpid();
    println!("started: {}", pid);

    let rtn = fib(40);

    println!("Result[{}] = {}", pid, rtn);

    println!("time[{}]: {} ms", pid, (time() - started).as_millis());
}
