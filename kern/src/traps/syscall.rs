use alloc::boxed::Box;
use core::time::Duration;

use crate::console::{CONSOLE, kprint};
use crate::process::State;
use crate::traps::TrapFrame;
use crate::SCHEDULER;
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

/// 現在地を返す.
///
/// このシステムコールはパラメタを取らない.
///
/// このシステムコールは通常の状態値に加えて次のパラメタを2つ返す::
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
        _ => unimplemented!("syscall {}", num),
    }
}
