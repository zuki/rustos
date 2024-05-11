use core::fmt;

use alloc::boxed::Box;

use crate::process::Process;

/// プロセスが再スケジューリングの準備ができているか否かを判断するために
/// 使用される関数の型。スケジューラはそのプロセスに実行順番が回ってきた
/// 際にこの関数を呼び出す。この関数が `true` を返した場合、そのプロセスが
/// スケジュールされる。`false` を返した場合、プロセスはスケジュールされず、
/// 次のタイムスライスで再度この関数が呼ばれることになる。
pub type EventPollFn = Box<dyn FnMut(&mut Process) -> bool + Send>;

/// プロセスのスケジューリング状態.
pub enum State {
    /// プロセスはスケジューリングされる準備ができている.
    Ready,
    /// プロセスはスケジュールされる前に必要なイベントの発生を待っている.
    Waiting(EventPollFn),
    /// プロセスは現在実行中である.
    Running,
    /// プロセスは現在死んでいる（再生可能）.
    Dead,
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            State::Ready => write!(f, "State::Ready"),
            State::Running => write!(f, "State::Running"),
            State::Waiting(_) => write!(f, "State::Waiting"),
            State::Dead => write!(f, "State::Dead"),
        }
    }
}
