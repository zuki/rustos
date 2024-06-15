use alloc::boxed::Box;
//use core::time::Duration;

use smoltcp::wire::{IpAddress, IpEndpoint};

use crate::console::kprint;
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

/// ソケットを作成してソケットハンドルをカレントプロセスの
/// ソケットリストに保存する.
///
/// この関数は引数を取らない。
///
/// このシステムコールは通常のステータス値に加えてソケットディスクリプタを
/// 返す。
///
/// NOTE: オリジナル注記には「今関数は通常のステータス値以外は何も
/// 返さない」とあるがソケットディスクリプタを返さないと以後この
/// ソケットは使えないはずなのでソケットディスクリプタを返す仕様にした
///
/// FIXME: SocketHandleの定義は`struct SocketHandle(usize)`である。
/// self.0をディスクリプタに使いたいがプライベートフィールドで
/// アクセスできない。ここではprocess.socketsのindexをディスクリプタ
/// として使うことにした。これは一度pushしたハンドルを削除されなければ
/// 問題ないが、削除されたら意味をなくしまう。close()システムコールは
/// 実装しなくても良いとあるので実装しなければ問題ないか?
pub fn sys_sock_create(tf: &mut TrapFrame) {
    // Lab 5 2.D
    SCHEDULER.critical(|scheduler| {
        let process = scheduler.find_process(tf);
        let handle = ETHERNET.add_socket();
        process.sockets.push(handle);
        tf.xn[0] = (process.sockets.len() - 1) as u64;
    });

    tf.xn[7] = OsError::Ok as u64;
}

/// ソケットのステータスを返す。
///
/// このシステムコールはソケットディスクリプタを第１引数として取る。
///
/// 通常のステータス値に加え、このシステムコールはキューイング
/// されているソケットのステータスを記述する４つのブール値を返す。
///
/// - x0: is_active
/// - x1: is_listening
/// - x2: can_send
/// - x3: can_recv
///
/// # エラー
/// 指定されたディスクリプタに対応するソケットが見つからなかった場合、
/// この関数は `OsError::InvalidSocket` を返す。
pub fn sys_sock_status(sock_idx: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    SCHEDULER.critical(|scheduler| {
        let process = scheduler.find_process(tf);
        if process.sockets.len() <= sock_idx {
            tf.xn[7] = OsError::InvalidSocket as u64;
            return;
        }
        let handle = process.sockets[sock_idx];
        ETHERNET.critical(|driver| {
            let socket = driver.get_socket(handle);
            tf.xn[0] = socket.is_active() as u64;
            tf.xn[1] = socket.is_listening() as u64;
            tf.xn[2] = socket.can_send() as u64;
            tf.xn[3] = socket.can_recv() as u64;
            tf.xn[7] = OsError::Ok as u64;
        });
    });
}

/// ソケットを使ってローカルエフェメラルポートをリモートIPエンドポイントに
/// 接続する。
///
/// このシステムコールは第1パラメタとしてソケットディスクリプタ、
/// 第2パラメタとしてビッグエンディアンのリモートのIPエンドポイント、
/// 第3パラメタとしてリモートエンドポイントのポート番号を取る。
///
/// `handle_syscall` はこの関数を呼び出す際にレジスタの値を読み込んで、
/// `Into<IpEndpoint>` を実装する構造体を作成する必要がある。
///
/// この関数は通常のステータス値だけを返す。
///
/// # エラー
/// この関数は次のエラーを返すことができる:
///
/// - `OsError::NoEntry`: エフェメラルポートを割り当てられなかった
/// - `OsError::InvalidSocket`: 指定のディスクリプタに対応するソケットが見つからなかった
/// - `OsError::IllegalSocketOperation`: `connect()`が `smoltcp::Error::Illegal` を返した
/// - `OsError::BadAddress`: `connect()` が `smoltcp::Error::Unaddressable` を返した
/// - `OsError::Unknown`: `connect()`の呼び出しによるその他のすべてのエラー
pub fn sys_sock_connect(
    sock_idx: usize,
    remote_endpoint: impl Into<IpEndpoint>,
    tf: &mut TrapFrame,
) {
    // Lab 5 2.D
    SCHEDULER.critical(|scheduler| {
        let process = scheduler.find_process(tf);
        if process.sockets.len() <= sock_idx {
            tf.xn[7] = OsError::InvalidSocket as u64;
            return;
        }
        let handle = process.sockets[sock_idx];

        let port = ETHERNET.critical(|driver| driver.get_ephemeral_port());
        if port.is_none() {
            tf.xn[7] = OsError::NoEntry as u64;
            return;
        }
        let port = port.unwrap();
        let ipaddr = ETHERNET.critical(|driver| driver.get_ipaddress());
        let local_endpoint = IpEndpoint::new(ipaddr, port);

        ETHERNET.critical(|driver| {
            let mut socket = driver.get_socket(handle);
            match socket.connect(remote_endpoint, local_endpoint) {
                Ok(_) => {
                    ETHERNET.critical(|driver| driver.mark_port(port));
                    tf.xn[7] = OsError::Ok as u64;
                }
                Err(smoltcp::Error::Illegal) => tf.xn[7] = OsError::IllegalSocketOperation as u64,
                Err(smoltcp::Error::Unaddressable) => tf.xn[7] = OsError::BadAddress as u64,
                Err(_) => tf.xn[7] = OsError::Unknown as u64,
            }
        });
    });
}

/// ローカルポートで着信接続をリッスンする。
///
/// このシステムコールは第1パラメタとしてソケットディスクリプタ、
/// 第2パラメタとしてリッスンするローカルポートを取る。
///
/// この関数は通常のステータス値だけを返す。
///
/// # エラー
/// この関数は次のエラーを返すことができる:
///
/// - `OsError::InvalidSocket`: 指定のディスクリプタに対応するソケットが見つからなかった
/// - `OsError::IllegalSocketOperation`: `listen()` が `smoltcp::Error::Illegal` を返した
/// - `OsError::BadAddress`: `listen()` が `smoltcp::Error::Unaddressable` を返した
/// - `OsError::Unknown`: `listen()`の呼び出しによるその他のすべてのエラー
pub fn sys_sock_listen(sock_idx: usize, local_port: u16, tf: &mut TrapFrame) {
    // Lab 5 2.D
    SCHEDULER.critical(|scheduler| {
        let process = scheduler.find_process(tf);
        if process.sockets.len() <= sock_idx {
            tf.xn[7] = OsError::InvalidSocket as u64;
            return;
        }
        let handle = process.sockets[sock_idx];
        let ipaddr = ETHERNET.critical(|driver| driver.get_ipaddress());
        let local_endpoint = IpEndpoint::new(ipaddr, local_port);
        ETHERNET.critical(|driver| {
            let mut socket = driver.get_socket(handle);
            match socket.listen(local_endpoint) {
                Ok(_) => tf.xn[7] = OsError::Ok as u64,
                Err(smoltcp::Error::Illegal) => tf.xn[7] = OsError::IllegalSocketOperation as u64,
                Err(smoltcp::Error::Unaddressable) => tf.xn[7] = OsError::BadAddress as u64,
                Err(_) => tf.xn[7] = OsError::Unknown as u64,
            }
        });
    });
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

/// 接続されたソケットを使ってデータを送信する.
///
/// このシステムコールは第1パラメタとしてソケットディスクリプタ、
/// 第2パラメタとしてバッファアドレス、
/// 第3パラメタとしてバッファ長を取る。
///
/// この関数は通常のステータス値に加え、送信したバイト長を返す。
///
/// # エラー
/// この関数は次のエラーを返すことができる:
///
/// - `OsError::InvalidSocket`: 指定のディスクリプタに対応するソケットが見つからなかった
/// - `OsError::BadAddress`: アドレスと長さのペアが有効なユーザアドレス空間のスライスとならない
/// - `OsError::IllegalSocketOperation`: `send_slice()` が `smoltcp::Error::Illegal` を返した。
/// - `OsError::Unknown`: smoltcpからのその他のすべてのエラー
pub fn sys_sock_send(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    SCHEDULER.critical(|scheduler| {
        let process = scheduler.find_process(tf);
        if process.sockets.len() <= sock_idx {
            tf.xn[7] = OsError::InvalidSocket as u64;
            return;
        }
        let handle = process.sockets[sock_idx];
        ETHERNET.critical(|driver| {
            let mut socket = driver.get_socket(handle);
            match unsafe { to_user_slice(va, len) } {
                Ok(data) => {
                    match socket.send_slice(data) {
                        Ok(size) => {
                            tf.xn[1] = size as u64;
                            tf.xn[7] = OsError::Ok as u64;
                        }
                        Err(smoltcp::Error::Illegal) => tf.xn[7] = OsError::IllegalSocketOperation as u64,
                        Err(_) => tf.xn[7] = OsError::Unknown as u64,
                    }
                }
                Err(error) => tf.xn[7] = error as u64,
            };
        });
    });
}

/// 接続されたソケットからデータを受診する.
///
/// このシステムコールは第1パラメタとしてソケットディスクリプタ、
/// 第2パラメタとしてバッファアドレス、
/// 第3パラメタとしてバッファ長を取る。
///
/// この関数は通常のステータス値に加え、受診したバイト長を返す。
///
/// # エラー
/// この関数は次のエラーを返すことができる:
///
/// - `OsError::InvalidSocket`: 指定のディスクリプタに対応するソケットが見つからなかった
/// - `OsError::BadAddress`: アドレスと長さのペアが有効なユーザアドレス空間のスライスとならない
/// - `OsError::IllegalSocketOperation`: `recv_slice()` が `smoltcp::Error::Illegal` を返した。
/// - `OsError::Unknown`: smoltcpからのその他のすべてのエラー
pub fn sys_sock_recv(sock_idx: usize, va: usize, len: usize, tf: &mut TrapFrame) {
    // Lab 5 2.D
    SCHEDULER.critical(|scheduler| {
        let process = scheduler.find_process(tf);
        if process.sockets.len() <= sock_idx {
            tf.xn[7] = OsError::InvalidSocket as u64;
            return;
        }
        let handle = process.sockets[sock_idx];
        ETHERNET.critical(|driver| {
            let mut socket = driver.get_socket(handle);
            match unsafe { to_user_slice_mut(va, len) } {
                Ok(data) => {
                    match socket.recv_slice(data) {
                        Ok(size) => {
                            tf.xn[1] = size as u64;
                            tf.xn[7] = OsError::Ok as u64;
                        }
                        Err(smoltcp::Error::Illegal) => tf.xn[7] = OsError::IllegalSocketOperation as u64,
                        Err(_) => tf.xn[7] = OsError::Unknown as u64,
                    }
                }
                Err(error) => tf.xn[7] = error as u64,
            };
        });
    });
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
    //use crate::console::kprintln;
    //kprintln!("handle_syscall: {}", num);
    //kprintln!("x0: {:X}", tf.xn[0]);
    match num as usize {
        NR_SLEEP => sys_sleep(tf.xn[0] as u32, tf),
        NR_TIME => sys_time(tf),
        NR_EXIT => sys_exit(tf),
        NR_WRITE => sys_write(tf.xn[0] as u8, tf),
        NR_GETPID => sys_getpid(tf),
        NR_WRITE_STR => sys_write_str(tf.xn[0] as usize, tf.xn[1] as usize, tf),
        NR_SOCK_CREATE => sys_sock_create(tf),
        NR_SOCK_STATUS => sys_sock_status(tf.xn[0] as usize, tf),
        NR_SOCK_CONNECT => {
            let ipaddr_be = tf.xn[1];
            let a0 = ((ipaddr_be & 0xff000000_00000000) >> 56) as u8;
            let a1 = ((ipaddr_be & 0x00ff0000_00000000) >> 48) as u8;
            let a2 = ((ipaddr_be & 0x0000ff00_00000000) >> 40) as u8;
            let a3 = ((ipaddr_be & 0x000000ff_00000000) >> 32) as u8;
            let ipaddr = IpAddress::v4(a0, a1, a2, a3);
            let port = tf.xn[2] as u16;
            sys_sock_connect(tf.xn[0] as usize, IpEndpoint::new(ipaddr, port), tf);
        }
        NR_SOCK_LISTEN => sys_sock_listen(tf.xn[0] as usize, tf.xn[1] as u16, tf),
        NR_SOCK_SEND => sys_sock_send(tf.xn[0] as usize, tf.xn[1] as usize, tf.xn[2] as usize, tf),
        NR_SOCK_RECV => sys_sock_recv(tf.xn[0] as usize, tf.xn[1] as usize, tf.xn[2] as usize, tf),
        _ => unimplemented!("syscall {}", num),
    }
}
