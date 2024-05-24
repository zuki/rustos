use alloc::boxed::Box;
use core::time::Duration;

use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::console::{kprint, CONSOLE};
use crate::param::USER_IMG_BASE;
use crate::process::State;
use crate::traps::TrapFrame;
use crate::{ETHERNET, SCHEDULER};

use kernel_api::*;
use pi::timer::current_time;

/// `ms` ミリ秒スリープする
///
/// このシステムコールは1つパラメタ: スリープするミリ秒数 を取る.
///
/// このシステムコールは通常の状態値に加えて次のパラメタを1つ返す:
///     `sleep`が呼び出されたときから`sleep`が復帰するまでの
///     おおよその真の経過時間.
pub fn sys_sleep(ms: u32, tf: &mut TrapFrame) {
    let started = current_time();
    SCHEDULER.switch(
        State::Waiting(Box::new(move |p| {
            let now = current_time();
            let elapsed = (now - started).as_millis() as u32;
            if elapsed >= ms {
                p.context.xn[0] = (now - started).as_millis() as u64;
                p.context.xn[7] = OsError::Ok as u64;
                true
            } else {
                false
            }
        })),
        tf,
    );
}

/// 現在時を返す.
///
/// このシステムコールはパラメタを取らない.
///
/// このシステムコールは通常の状態値に加えて次のパラメタを2つ返す:
///  - 現在時（秒単位）
///  - 現在時の分数部分（ナノ秒単位）.
pub fn sys_time(tf: &mut TrapFrame) {
    let now = current_time();
    tf.xn[0] = now.as_secs();
    tf.xn[1] = now.subsec_nanos() as u64;
    tf.xn[7] = OsError::Ok as u64;
}

/// カレントプロセスをkillする.
///
/// このシステムコールはパラメタを取らず、どのような値も返さない.
pub fn sys_exit(tf: &mut TrapFrame) {
    let _ = SCHEDULER.kill(tf);
    let _ = SCHEDULER.switch_to(tf);
}

/// コンソールに書き出す.
///
/// このシステムコールはパラメタを1つ受け取る: 出力する u8 文字.
///
/// 通常の状態値だけを返す.
pub fn sys_write(b: u8, tf: &mut TrapFrame) {
    kprint!{"{}", b as char};
    tf.xn[7] = OsError::Ok as u64;
}

/// カレントプロセスのIDを返す.
///
/// このシステムコールはパラメタを取らない.
///
/// このシステムコールは通常の状態値に加えて次のパラメタを1つ返す:
///     カレントプロセスのID.
pub fn sys_getpid(tf: &mut TrapFrame) {
    tf.xn[0] = tf.tpidr;
    tf.xn[7] = OsError::Ok as u64;
}

/// Creates a socket and saves the socket handle in the current process's
/// socket list.
///
/// This function does neither take any parameter nor return anything,
/// except the usual return code that indicates successful syscall execution.
pub fn sys_sock_create(tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_create")
}

/// Returns the status of a socket.
///
/// This system call takes a socket descriptor as the first parameter.
///
/// In addition to the usual status value, this system call returns four boolean
/// values that describes the status of the queried socket.
///
/// - x0: is_active
/// - x1: is_listening
/// - x2: can_send
/// - x3: can_recv
///
/// # Errors
/// This function returns `OsError::InvalidSocket` if a socket that corresponds
/// to the provided descriptor is not found.
pub fn sys_sock_status(sock_idx: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_status")
}

/// Connects a local ephemeral port to a remote IP endpoint with a socket.
///
/// This system call takes a socket descriptor as the first parameter, the IP
/// of the remote endpoint as the second paramter in big endian, and the port
/// number of the remote endpoint as the third parameter.
///
/// `handle_syscall` should read the value of registers and create a struct that
/// implements `Into<IpEndpoint>` when calling this function.
///
/// It only returns the usual status value.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::NoEntry`: Fails to allocate an ephemeral port
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::IllegalSocketOperation`: `connect()` returned `smoltcp::Error::Illegal`.
/// - `OsError::BadAddress`: `connect()` returned `smoltcp::Error::Unaddressable`.
/// - `OsError::Unknown`: All the other errors from calling `connect()`.
pub fn sys_sock_connect(
    sock_idx: usize,
    remote_endpoint: impl Into<IpEndpoint>,
    tf: &mut TrapFrame,
) {
    // Lab 5 2.D
    unimplemented!("sys_sock_connect")
}

/// Listens on a local port for an inbound connection.
///
/// This system call takes a socket descriptor as the first parameter and the
/// local ports to listen on as the second parameter.
///
/// It only returns the usual status value.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::IllegalSocketOperation`: `listen()` returned `smoltcp::Error::Illegal`.
/// - `OsError::BadAddress`: `listen()` returned `smoltcp::Error::Unaddressable`.
/// - `OsError::Unknown`: All the other errors from calling `listen()`.
pub fn sys_sock_listen(sock_idx: usize, local_port: u16, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_listen")
}

/// 仮想アドレスと長さからスライスを返す.
///
/// # エラー
/// この関数はスライスが完全にユーザ空間にない場合は
/// `Err(OsError::BadAddress)`を返す。
unsafe fn to_user_slice<'a>(va: usize, len: usize) -> OsResult<&'a [u8]> {
    let overflow = va.checked_add(len).is_none();
    if va >= USER_IMG_BASE && !overflow {
        Ok(core::slice::from_raw_parts(va as *const u8, len))
    } else {
        Err(OsError::BadAddress)
    }
}
/// 仮想アドレスと長さから可変スライスを返す.
///
/// # エラー
/// この関数はスライスが完全にユーザ空間にない場合は
/// `Err(OsError::BadAddress)`を返す。
unsafe fn to_user_slice_mut<'a>(va: usize, len: usize) -> OsResult<&'a mut [u8]> {
    let overflow = va.checked_add(len).is_none();
    if va >= USER_IMG_BASE && !overflow {
        Ok(core::slice::from_raw_parts_mut(va as *mut u8, len))
    } else {
        Err(OsError::BadAddress)
    }
}

/// Sends data with a connected socket.
///
/// This system call takes a socket descriptor as the first parameter, the
/// address of the buffer as the second parameter, and the length of the buffer
/// as the third parameter.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the number of bytes sent.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::IllegalSocketOperation`: `send_slice()` returned `smoltcp::Error::Illegal`.
/// - `OsError::Unknown`: All the other errors from smoltcp.
pub fn sys_sock_send(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_send")
}

/// Receives data from a connected socket.
///
/// This system call takes a socket descriptor as the first parameter, the
/// address of the buffer as the second parameter, and the length of the buffer
/// as the third parameter.
///
/// In addition to the usual status value, this system call returns one
/// parameter: the number of bytes read.
///
/// # Errors
/// This function can return following errors:
///
/// - `OsError::InvalidSocket`: Cannot find a socket that corresponds to the provided descriptor.
/// - `OsError::BadAddress`: The address and the length pair does not form a valid userspace slice.
/// - `OsError::IllegalSocketOperation`: `recv_slice()` returned `smoltcp::Error::Illegal`.
/// - `OsError::Unknown`: All the other errors from smoltcp.
pub fn sys_sock_recv(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    unimplemented!("sys_sock_recv")
}

/// UTF-8文字列をコンソールに出力する.
///
/// このシステムコールは第1パラメタとしてバッファのアドレスを、
/// 第2パラメタとしてバッファの長さを受け取る.
///
/// このシステムコールは通常の状態値に加えてUTF-8メッセージの長さを返す.
///
/// # エラー
/// この関数は次のエラーを返すことができる:
///
/// - `OsError::BadAddress`: アドレスと長さの組が正しいユーザ空間スライスを形成しない.
/// - `OsError::InvalidArgument`: 指定のバッファは UTF-8 エンコードでない.
pub fn sys_write_str(va: usize, len: usize, tf: &mut TrapFrame) {
    let result = unsafe { to_user_slice(va, len) }
        .and_then(|slice| core::str::from_utf8(slice).map_err(|_| OsError::InvalidArgument));

    match result {
        Ok(msg) => {
            kprint!("{}", msg);

            tf.xn[0] = msg.len() as u64;
            tf.xn[7] = OsError::Ok as u64;
        }
        Err(e) => {
            tf.xn[7] = e as u64;
        }
    }
}

// システムコールを処理する
pub fn handle_syscall(num: u16, tf: &mut TrapFrame) {
    use crate::console::kprintln;
    //kprintln!("handle_syscall: {}", num);
    //kprintln!("x0: {:X}", tf.xn[0]);
    match num as usize {
        NR_SLEEP => sys_sleep(tf.xn[0] as u32, tf),
        NR_TIME => sys_time(tf),
        NR_EXIT => sys_exit(tf),
        NR_WRITE => sys_write(tf.xn[0] as u8, tf),
        NR_GETPID => sys_getpid(tf),
        NR_WRITE_STR => sys_write_str(tf.xn[0] as usize, tf.xn[1] as usize, tf),
        _ => unimplemented!("syscall {}", num),
    }
}
