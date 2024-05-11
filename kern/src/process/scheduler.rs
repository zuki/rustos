use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::fmt;

use aarch64::*;

use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::TrapFrame;
use crate::VMM;

/// マシン全体用のプロセススケジューラ.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Scheduler>>);

impl GlobalScheduler {
    /// 初期化していないローカルスケジューラのラッパーを返す.
    pub const fn uninitialized() -> GlobalScheduler {
        GlobalScheduler(Mutex::new(None))
    }

    /// クリティカルリージョンに入り、内部スケジューラで指定の
    /// クロージャを実行する.
    pub fn critical<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Scheduler) -> R,
    {
        let mut guard = self.0.lock();
        f(guard.as_mut().expect("scheduler uninitialized"))
    }


    /// プロセスをスケジューラのキューに追加し、そのプロセスのIDを返す
    /// 詳細はの `Scheduler::add()` のドキュメントを参照。
    pub fn add(&self, process: Process) -> Option<Id> {
        self.critical(move |scheduler| scheduler.add(process))
    }

    /// 現在のプロセスの状態を `new_state` に設定し、 `tf` を現在の
    /// プロセスに保存し、次のプロセスのトラップフレームを `tf` に
    /// 復元することにより `tf` を使用してコンテキストスイッチを実行する。
    /// 詳細は `Scheduler::schedule_out()` と `Scheduler::switch_to()` の
    /// ドキュメントを参照。
    pub fn switch(&self, new_state: State, tf: &mut TrapFrame) -> Id {
        self.critical(|scheduler| scheduler.schedule_out(new_state, tf));
        self.switch_to(tf)
    }

    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                return id;
            }
            aarch64::wfe();
        }
    }

    /// 現在実行中のプロセスをkillし、そのプロセスのIDを返す.
    /// 詳細は `Scheduler::kill()` のドキュメントを参照。
    #[must_use]
    pub fn kill(&self, tf: &mut TrapFrame) -> Option<Id> {
        self.critical(|scheduler| scheduler.kill(tf))
    }

    /// タイマー割り込みベースのプリエンプションスケジューリングを
    /// 使ってユーザ空間のプロセスの実行を開始する。このメソッドは
    /// 通常の条件では復帰しない。
    pub fn start(&self) -> ! {
        let process = Process::new().expect("new process");
        let mut tf = process.context;
        // 例外からの戻り先はstart_shell()関数
        tf.elr = start_shell as *const u64 as u64;
        // SPSR_EL1をセット。IRQはアンマスク
        tf.spsr = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        // SP_EL0はこのプロセスのスタックのtop
        tf.sp = process.stack.top().as_u64();
        // 最初のプロセスなので1をセット
        tf.tpidr = 1;

        // spにトラップフレームをセットしてcontext_restore
        unsafe {
            asm!("mov x0, $0
                  mov sp, x0"
                 :: "r"(tf)
                 :: "volatile");
            asm!("bl context_restore" :::: "volatile");
            asm!("adr x0, _start
                  mov sp, x0"
                 :::: "volatile");
            asm!("mov x0, #0" :::: "volatile");
        }
        eret();
        loop {};
    }

    /// スケジューラを初期化してユーザ空間のプロセスをスケジューラに追加する.
    pub unsafe fn initialize(&self) {
        unimplemented!("GlobalScheduler::initialize()")
    }

    // 次のメソッドはフェーズ3のテストに役に立つだろう。
    //
    // * extern関数をユーザプロセスのページテーブルにロードするメソッド.
    //
    // pub fn test_phase_3(&self, proc: &mut Process){
    //     use crate::vm::{VirtualAddr, PagePerm};
    //
    //     let mut page = proc.vmap.alloc(
    //         VirtualAddr::from(USER_IMG_BASE as u64), PagePerm::RWX);
    //
    //     let text = unsafe {
    //         core::slice::from_raw_parts(test_user_process as *const u8, 24)
    //     };
    //
    //     page[0..24].copy_from_slice(text);
    // }
}

#[derive(Debug)]
pub struct Scheduler {
    processes: VecDeque<Process>,
    last_id: Option<Id>,
}

impl Scheduler {
    /// からのキューを持つ新しい `Scheduler` を返す.
    fn new() -> Scheduler {
        unimplemented!("Scheduler::new()")
    }

    /// プロセスをスケジューラのキューに追加し、新しいプロセスが
    /// スケジューリング可能であればそのプロセスのIDを返す。
    /// プロセスIDはそのプロセスに新しく割り当てられたものであり、
    /// `trap_frame` に保存されている。スケジュールできるプロセスが
    /// ない場合は `None` を返す。
    ///
    /// 最初の `switch` の呼び出しとそのプロセスがCPU上で実行される
    /// ようにすることは呼び出し側の責任である。
    fn add(&mut self, mut process: Process) -> Option<Id> {
        unimplemented!("Scheduler::add()")
    }

    /// 現在実行中のプロセスを見つけ、現在のプロセスの状態を `new_state` に
    /// セットし、`tf` を現在のプロセスにセーブして `tf` によるコンテキスト
    /// スイッチを準備し、現在のプロセスを `processes` キューの末尾に
    /// プッシュする。
    ///
    /// プロセス `processes` キューが空であるか、現在のプロセスが存在しない
    /// 場合は `false` を返す。それ以外の場合は `true` を返す。
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        unimplemented!("Scheduler::schedule_out()")
    }

    /// 次に切り替えるべきプロセスを見つけ、その次のプロセスを `processes`
    /// キューの先頭に持ってきて、次のプロセスの状態を `Running` に変更し、
    /// 次のプロセスのトラップフレームを `tf` に復元することでコンテキスト
    /// スイッチを行う。
    ///
    /// 切り替えるプロセスがない場合は `None` を返す。そうでない場合は、
    /// 次のプロセスのプロセス IDの `Some` を返す。
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        unimplemented!("Scheduler::switch_to()")
    }

    /// 現在のプロセスを `Dead` 状態としてスケジューリングから外すことで
    /// 現在実行中のプロセスをkillする。死んだプロセスをキューから削除し、
    /// 死んだプロセスのインスタンスをdropし、死んだプロセスのプロセスIDを
    /// 返す。
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        unimplemented!("Scheduler::kill()")
    }
}

pub extern "C" fn  test_user_process() -> ! {
    loop {
        let ms = 10000;
        let error: u64;
        let elapsed_ms: u64;

        unsafe {
            asm!("mov x0, $2
              svc 1
              mov $0, x0
              mov $1, x7"
                 : "=r"(elapsed_ms), "=r"(error)
                 : "r"(ms)
                 : "x0", "x7"
                 : "volatile");
        }
    }
}

pub extern "C" fn start_shell() -> ! {
    use crate::shell;

    unsafe { asm!("brk 1" :::: "volatile"); }
    unsafe { asm!("brk 2" :::: "volatile"); }
    shell::shell("user0> ");
    unsafe { asm!("brk 3" :::: "volatile"); }
    loop {
        shell::shell("user1> ");
    }
}
