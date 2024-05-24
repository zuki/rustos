use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;

use core::borrow::Borrow;
use core::ffi::c_void;
use core::fmt;
use core::mem;
use core::time::Duration;

use aarch64::*;
use pi::local_interrupt::LocalInterrupt;
use smoltcp::time::Instant;

use crate::mutex::Mutex;
use crate::net::uspi::TKernelTimerHandle;
use crate::param::*;
use crate::percore::{get_preemptive_counter, is_mmu_ready, local_irq};
use crate::process::{Id, Process, State};
use crate::traps::irq::GlobalIrq;
use crate::traps::irq::IrqHandlerRegistry;
use crate::traps::TrapFrame;
use crate::{ETHERNET, USB};

use crate::traps::irq;
use crate::VMM;
use crate::SCHEDULER;
use crate::GLOABAL_IRQ;
use crate::console::{kprint, kprintln};
use pi::timer;
use pi::interrupt::{Interrupt, Controller};

/// マシン全体用のプロセススケジューラ.
#[derive(Debug)]
pub struct GlobalScheduler(Mutex<Option<Box<Scheduler>>>);

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

    /// Loops until it finds the next process to schedule.
    /// Call `wfi()` in the loop when no process is ready.
    /// For more details, see the documentation on `Scheduler::switch_to()`.
    ///
    /// Returns the process's ID when a ready process is found.
    pub fn switch_to(&self, tf: &mut TrapFrame) -> Id {
        loop {
            let rtn = self.critical(|scheduler| scheduler.switch_to(tf));
            if let Some(id) = rtn {
                trace!(
                    "[core-{}] switch_to {:?}, pc: {:x}, lr: {:x}, x29: {:x}, x28: {:x}, x27: {:x}",
                    affinity(),
                    id,
                    tf.elr,
                    tf.xn[30],
                    tf.xn[29],
                    tf.xn[28],
                    tf.xn[27]
                );
                return id;
            }
            aarch64::wfi();
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
        self.initialize_global_timer_interrupt();

        let mut tf = Box::new(TrapFrame::default());
        self.critical(|scheduler| scheduler.switch_to(&mut tf));

        //kprintln!("tf\n{:?}", tf);
        // 次のページを計算してspにセットする
        let new_sp = KERN_STACK_BASE - PAGE_SIZE;
        unsafe {
            asm!("mov x0, $0
                  mov sp, x0"
                 :: "r"(tf)
                 :: "volatile");
            asm!("bl context_restore" :::: "volatile");
            asm!("mov x0, $0
                  mov sp, x0"
                 :: "i"(new_sp)
                 :: "volatile");
            asm!("mov x0, xzr"
                 :::: "volatile");
            eret();
        }

        loop {};
    }



    /// スケジューラを初期化してユーザ空間のプロセスをスケジューラに追加する.
    /// # Lab 4
    /// グローバルタイマー割り込みを `pi::timer` で初期化する。タイマーは
    /// `param.rs` で定義された `TICK` 時間ごとに `Timer1` 割り込みが発生
    /// するように設定する必要がある。
    ///
    /// # Lab 5
    /// 1 秒後に `poll_ethernet` を起動するタイマーハンドラを
    /// `Usb::start_kernel_timer` に登録する.
    pub fn initialize_global_timer_interrupt(&self) {
        // タイマー割り込みハンドラの設定
        GLOABAL_IRQ.register(
            Interrupt::Timer1,
            Box::new(|tf| {
                timer::tick_in(TICK);
                let old_id = tf.tpidr;
                let id = SCHEDULER.switch(State::Ready, tf);
                //kprintln!("TICK, switch from {} to {}", old_id, id);
            }),
        );

        // タイマー割り込みの有効化
        timer::tick_in(TICK);
        let mut controller = Controller::new();
        controller.enable(Interrupt::Timer1);

    }

    /// Initializes the per-core local timer interrupt with `pi::local_interrupt`.
    /// The timer should be configured in a way that `CntpnsIrq` interrupt fires
    /// every `TICK` duration, which is defined in `param.rs`.
    pub fn initialize_local_timer_interrupt(&self) {
        // Lab 5 2.C
        unimplemented!("initialize_local_timer_interrupt()")
    }

    /// Initializes the scheduler and add userspace processes to the Scheduler.
    pub unsafe fn initialize(&self) {
        let mut scheduler = Scheduler::new();

        for _ in 0..4 {
            let p = Process::load("/fib").expect("load /fib");
            scheduler.add(p);
        }

        *self.0.lock() = Some(scheduler);

    }

    // 次のメソッドはフェーズ3のテストに役に立つだろう。
    //
    // * extern関数をユーザプロセスのページテーブルにロードするメソッド.
    //
    pub fn test_phase_3(&self, proc: &mut Process) {
        use crate::vm::{VirtualAddr, PagePerm};

        let mut page = proc.vmap.alloc(
            VirtualAddr::from(USER_IMG_BASE as u64), PagePerm::RWX);

            let text = unsafe {
            core::slice::from_raw_parts(test_user_process as *const u8, 24)
        };
        //kprintln!("proc.tf\n{:?}", proc.context);
        page[0..24].copy_from_slice(text);
        // ユーザページテーブルのデバッグ出力
        //kprint!("{:?}", &proc.vmap);
    }

}

/// Poll the ethernet driver and re-register a timer handler using
/// `Usb::start_kernel_timer`.
extern "C" fn poll_ethernet(_: TKernelTimerHandle, _: *mut c_void, _: *mut c_void) {
    // Lab 5 2.B
    unimplemented!("poll_ethernet")
}

/// Internal scheduler struct which is not thread-safe.
pub struct Scheduler {
    /// プロセスキュー
    processes: VecDeque<Process>,
    // プロセスID付番用
    last_id: Option<Id>,
}

impl Scheduler {
    /// 空のキューを持つ新しい `Scheduler` を返す.
    fn new() -> Box<Scheduler> {
        Box::new(Scheduler {
            processes: VecDeque::<Process>::new(),
            last_id: None,
        })
    }

    /// プロセスをスケジューラのキューに追加し、新しいプロセスが
    /// スケジューリング可能であればそのプロセスにプロセスIDを
    /// 新しく割り当て、`trap_frame` に保存して、そのIDを返す。
    /// スケジュールできない場合は `None` を返す。
    ///
    /// 最初の `switch` の呼び出しとそのプロセスがCPU上で実行される
    /// ようにすることは呼び出し側の責任である。
    fn add(&mut self, mut process: Process) -> Option<Id> {
        let id = match self.last_id {
            Some(core::u64::MAX) => {
                return None;
            }
            Some(last_id) => last_id + 1,
            None => 1,
        };
        self.last_id = Some(id);
        process.context.tpidr = id;
        //kprintln!("process {} added", id);
        self.processes.push_back(process);
        Some(id)
    }

    /// 現在実行中のプロセスを見つけ、現在のプロセスの状態を `new_state` に
    /// 変更し、`tf` を現在のプロセスにセーブして `tf` によるコンテキスト
    /// スイッチを準備した後、現在のプロセスを `processes` キューの末尾に
    /// 移動させる。
    ///
    /// プロセス `processes` キューが空であるか、現在のプロセスが存在しない
    /// 場合は `false` を返す。それ以外の場合は `true` を返す。
    fn schedule_out(&mut self, new_state: State, tf: &mut TrapFrame) -> bool {
        match self.current_process(tf) {
            Some(index) => {
                let mut process = self.processes.remove(index).unwrap();
                process.state = new_state;
                process.context = Box::new(*tf);
                self.processes.push_back(process);
                true
            }
            None => false
        }
    }

    /// 次に切り替えるべきプロセスを見つけ、そのプロセスを `processes`
    /// キューの先頭に移動させ、状態を `Running` に変更し、トラップ
    /// フレームを `tf` に復元することでコンテキストスイッチを行う。
    ///
    /// 切り替えるプロセスがない場合は `None` を返す。そうでない場合は、
    /// 切り替えるプロセスのプロセス IDの `Some` を返す。
    fn switch_to(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        let mut index: usize = 0;
        for p in self.processes.iter_mut() {
            if p.is_ready() {
                break;
            } else {
                index += 1;
            }
        }
        // 切り替えるプロセスがない
        if index == self.processes.len() {
            return None;
        }
        //kprintln!("sw_to_before.tf\n{:?}", &tf);
        let mut process = self.processes.remove(index).unwrap();
        process.state = State::Ready;
        *tf = *process.context;
        let id = process.context.tpidr;
        //kprintln!("sw_to_after.tf\n{:?}", &tf);
        self.processes.push_front(process);
        Some(id)
    }

    /// 現在のプロセスを `Dead` 状態としてスケジューリングから外すことで
    /// 現在実行中のプロセスをkillする。死んだプロセスをキューから削除し、
    /// 死んだプロセスのインスタンスをdropし、死んだプロセスのプロセスIDを
    /// 返す。
    fn kill(&mut self, tf: &mut TrapFrame) -> Option<Id> {
        match self.current_process(tf) {
            Some(index) => {
                let mut process = self.processes.remove(index).unwrap();
                process.state = State::Dead;
                Some(process.context.tpidr)
            }
            None => None,
        }
    }

    /// カレントプロセスのキューでのインデックスを返す
    fn current_process(&mut self, tf: &mut TrapFrame) -> Option<usize> {
        let mut index: usize = 0;
        for p in self.processes.iter_mut() {
            if p.context.tpidr == tf.tpidr {
                break;
            } else {
                index += 1;
            }
        }
        // カレントプロセスがない
        if index == self.processes.len() {
            None
        } else {
            Some(index)
        }
    }

    /// Releases all process resources held by the current process such as sockets.
    fn release_process_resources(&mut self, tf: &mut TrapFrame) {
        // Lab 5 2.C
        unimplemented!("release_process_resources")
    }

    /// Finds a process corresponding with tpidr saved in a trap frame.
    /// Panics if the search fails.
    pub fn find_process(&mut self, tf: &TrapFrame) -> &mut Process {
        for i in 0..self.processes.len() {
            if self.processes[i].context.tpidr == tf.tpidr {
                return &mut self.processes[i];
            }
        }
        panic!("Invalid TrapFrame");
    }
}

impl fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let len = self.processes.len();
        write!(f, "  [Scheduler] {} processes in the queue\n", len)?;
        for i in 0..len {
            write!(
                f,
                "    queue[{}]: proc({:3})-{:?} \n",
                i, self.processes[i].context.tpidr, self.processes[i].state
            )?;
        }
        Ok(())
    }
}

pub extern "C" fn  test_user_process() -> ! {
    loop {
        let ms = 3000;
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
