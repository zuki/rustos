use core::fmt;
use core::fmt::Write;
use core::time::Duration;

use crate::*;

macro_rules! err_or {
    ($ecode:expr, $rtn:expr) => {{
        let e = OsError::from($ecode);
        if let OsError::Ok = e {
            Ok($rtn)
        } else {
            Err(e)
        }
    }};
}

pub fn sleep(span: Duration) -> OsResult<Duration> {
    if span.as_millis() > core::u64::MAX as u128 {
        panic!("too big!");
    }

    let ms = span.as_millis() as u64;
    let mut ecode: u64;
    let mut elapsed_ms: u64;

    unsafe {
        asm!("mov x0, $2
              svc $3
              mov $0, x0
              mov $1, x7"
             : "=r"(elapsed_ms), "=r"(ecode)
             : "r"(ms), "i"(NR_SLEEP)
             : "x0", "x7"
             : "volatile");
    }

    err_or!(ecode, Duration::from_millis(elapsed_ms))
}

pub fn time() -> Duration {
    let mut secs: u64;
    let mut nanos: u64;
    let mut ecode: u64;

    unsafe {
        asm!("svc $3
              mov $0, x0
              mov $1, x1
              mov $2, x7"
             : "=r"(secs), "=r"(nanos), "=r"(ecode)
             : "i"(NR_TIME)
             : "x0", "x1", "x7"
             : "volatile");
    }

    Duration::new(secs, nanos as u32)
}

pub fn exit() -> ! {
    unsafe {
        asm!("svc $0"
             :: "i"(NR_EXIT)
             :: "volatile");
    }
    loop {}
}

pub fn write(b: u8) {
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $1
              svc $2
              mov $0, x7"
             : "=r"(ecode)
             : "r"(b), "i"(NR_WRITE)
             : "x7"
             : "volatile");
    }
}

pub fn write_str(msg: &str) {
    let mut ecode: u64;
    let mut ulen: u64;
    let ptr = msg.as_ptr() as u64;
    let len = msg.len();

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $4
              mov $0, x0
              mov $1, x7"
             : "=r"(ulen), "=r"(ecode)
             : "r"(ptr), "r"(len), "i"(NR_WRITE_STR)
             : "x0", "x7"
             : "volatile");
    }
}

pub fn getpid() -> u64 {
    let mut pid: u64;
    let mut ecode: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(pid), "=r"(ecode)
             : "i"(NR_GETPID)
             : "x0", "x7"
             : "volatile");
    }
    pid
}

pub fn sock_create() -> SocketDescriptor {
    // Lab 5 2.D
    let mut descriptor;
    let mut ecode: u64;

    unsafe {
        asm!("svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(descriptor), "=r"(ecode)
             : "i"(NR_SOCK_CREATE)
             : "x0", "x7"
             : "volatile");
    }
    SocketDescriptor(descriptor)
}

pub fn sock_status(descriptor: SocketDescriptor) -> OsResult<SocketStatus> {
    // Lab 5 2.D
    let mut is_active: bool;
    let mut is_listening: bool;
    let mut can_send: bool;
    let mut can_recv: bool;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $6
              svc $5
              mov $0, x0
              mov $1, x1
              mov $2, x2
              mov $3, x3
              mov $4, x7"
             : "=r"(is_active), "=r"(is_listening), "=r"(can_send), "=r"(can_recv), "=r"(ecode)
             : "i"(NR_SOCK_CREATE), "r"(descriptor.raw())
             : "x0", "x1", "x2", "x3", "x7"
             : "volatile");
    }
    err_or!(ecode, SocketStatus {is_active, is_listening, can_send, can_recv})
}

pub fn sock_connect(descriptor: SocketDescriptor, addr: IpAddr) -> OsResult<()> {
    // Lab 5 2.D
    let ip = addr.ip.to_be() as u64;
    let port = addr.port as u64;

    let mut ecode: u64;
    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              mov x2, $4
              svc $1
              mov $0, x7"
             : "=r"(ecode)
             : "i"(NR_SOCK_CONNECT), "r"(descriptor.raw()), "r"(ip), "r"(port)
             : "x0", "x1", "x2", "x7"
             : "volatile");
    }
    err_or!(ecode, ())
}

pub fn sock_listen(descriptor: SocketDescriptor, local_port: u16) -> OsResult<()> {
    // Lab 5 2.D
    let port = local_port as u64;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $2
              mov x1, $3
              svc $1
              mov $0, x7"
             : "=r"(ecode)
             : "i"(NR_SOCK_LISTEN), "r"(descriptor.raw()), "r"(port)
             : "x0", "x1", "x7"
             : "volatile");
    }
    err_or!(ecode, ())
}

pub fn sock_send(descriptor: SocketDescriptor, buf: &[u8]) -> OsResult<usize> {
    // Lab 5 2.D
    let buf_addr = buf.as_ptr() as u64;
    let buf_len = buf.len();
    let mut size: u64;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $3
              mov x1, $4
              mov x2, $5
              svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(size), "=r"(ecode)
             : "i"(NR_SOCK_SEND), "r"(descriptor.raw()), "r"(buf_addr), "r"(buf_len)
             : "x0", "x1", "x2", "x7"
             : "volatile");
    }
    err_or!(ecode, size as usize)
}

pub fn sock_recv(descriptor: SocketDescriptor, buf: &mut [u8]) -> OsResult<usize> {
    // Lab 5 2.D
    let buf_addr = buf.as_mut_ptr() as u64;
    let buf_len = buf.len();
    let mut size: u64;
    let mut ecode: u64;

    unsafe {
        asm!("mov x0, $3
              mov x1, $4
              mov x2, $5
              svc $2
              mov $0, x0
              mov $1, x7"
             : "=r"(size), "=r"(ecode)
             : "i"(NR_SOCK_RECV), "r"(descriptor.raw()), "r"(buf_addr), "r"(buf_len)
             : "x0", "x1", "x2", "x7"
             : "volatile");
    }
    err_or!(ecode, size as usize)
}

struct Console;

impl fmt::Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        write_str(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::syscall::vprint(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
 () => (print!("\n"));
    ($($arg:tt)*) => ({
        $crate::syscall::vprint(format_args!($($arg)*));
        $crate::print!("\n");
    })
}

pub fn vprint(args: fmt::Arguments) {
    let mut c = Console;
    c.write_fmt(args).unwrap();
}
