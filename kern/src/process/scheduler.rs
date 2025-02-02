use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use alloc::vec::Vec;
use pi::timer::current_time;

//use core::borrow::Borrow;
use core::ffi::c_void;
use core::fmt;
use core::mem;
use core::time::Duration;

use aarch64::*;
use pi::local_interrupt::{LocalInterrupt, LocalController, local_tick_in};
use pi::interrupt::{Controller, Interrupt};
use pi::timer;
use smoltcp::time::Instant;

use crate::mutex::Mutex;
use crate::net::uspi::TKernelTimerHandle;
use crate::net::GlobalEthernetDriver;
use crate::param::*;
use crate::percore::{get_preemptive_counter, is_mmu_ready, local_irq};
use crate::process::{Id, Process, State};
//use crate::traps::irq::GlobalIrq;
use crate::traps::irq::IrqHandlerRegistry;
use crate::traps::TrapFrame;
use crate::GLOBAL_IRQ;
use crate::{ETHERNET, USB};

//use crate::traps::irq;
//use crate::VMM;
use crate::SCHEDULER;
//use crate::GLOBAL_IRQ;
//use crate::console::{kprint, kprintln};
//use pi::timer;
//use pi::interrupt::{Interrupt, Controller};

extern "C" {
    fn _start();
    fn context_restore();
}

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
            /*
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
            */
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
        if affinity() == 0 {
            self.initialize_global_timer_interrupt();
        }
        self.initialize_local_timer_interrupt();

        let mut tf = Box::new(TrapFrame::default());
        enable_fiq_interrupt();
        self.critical(|scheduler| scheduler.switch_to(&mut tf));
        disable_fiq_interrupt();

        //kprintln!("tf\n{:?}", tf);
        // 次のページを計算してspにセットする
        let mut cur_sp = SP.get();

        unsafe {
            asm!("mov x28, $0
                  mov x29, $1
                  mov sp, x28"
                 :: "r"(tf), "r"(cur_sp)
                 :: "volatile");
            asm!("bl context_restore" :::: "volatile");
            asm!("mov $0, x29" : "=r"(cur_sp) ::: "volatile");
            asm!("ldp x28, x29, [SP], #16
                  ldp lr,  xzr, [SP], #16
                  mov sp, $0"
                 :: "r"(cur_sp) :: "volatile");

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
        // 1. グローバルタイマー割り込みの設定
        let mut controller = Controller::new();
        controller.enable(Interrupt::Timer1);

        GLOBAL_IRQ.register(
            Interrupt::Timer1,
            Box::new(|tf| {
                timer::tick_in(TICK);
                SCHEDULER.switch(State::Ready, tf);
            }),
        );
        timer::tick_in(TICK);


        // 2. poll_ethernet起動タイマーハンドラの設定
        USB.start_kernel_timer(Duration::from_secs(1), Some(poll_ethernet));
    }

    /// `pi::local_interrupt`を使ってper-coreローカルタイマーを初期化する.
    /// タイマーは`CntpnsIrq`が`param.rs`で定義されている毎`TICK`間隔で
    /// 発火すように設定する必要がある.
    pub fn initialize_local_timer_interrupt(&self) {
        // Lab 5 2.C
        let mut controller = LocalController::new(affinity());
        controller.enable_local_timer();

        local_irq().register(
	        LocalInterrupt::CNTPNSIRQ,
      	    Box::new(|tf| {
                local_tick_in(affinity(), TICK);
                SCHEDULER.switch(State::Ready, tf);
            }),
        );

        controller.tick_in(TICK);
    }

    /// スケジューラを初期化してユーザ空間プロセスをスケジューラに追加する.
    pub unsafe fn initialize(&self) {
        let scheduler = Scheduler::new();
        *self.0.lock() = Some(scheduler);

    /*
        for _ in 0..3 {
            let p = Process::load("/fib").expect("load /fib");
            self.add(p);
        }
    */
        let p = Process::load("/echo").expect("load /echo");
        self.add(p);
    /*
        let p = Process::load("/fib_20").expect("load /fib_20");
        self.add(p);
        let p = Process::load("/fib_25").expect("load /fib_25");
        self.add(p);
        let p = Process::load("/fib_30").expect("load /fib_30");
        self.add(p);
    */
    }

    // 次のメソッドはフェーズ3のテストに役に立つだろう。
    //
    // * extern関数をユーザプロセスのページテーブルにロードするメソッド.
    //
    pub fn test_phase_3(&self, proc: &mut Process) {
        use crate::vm::{VirtualAddr, PagePerm};

        let page = proc.vmap.alloc(
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

/// Ethernetドライバをポーリングし、`Usb::start_kernel_timer`を使って
/// タイマーハンドラを再登録する
/// .
extern "C" fn poll_ethernet(_: TKernelTimerHandle, _: *mut c_void, _: *mut c_void) {
    // Lab 5 2.B
    ETHERNET.poll(Instant::from_millis(current_time().as_millis() as i64));
    let delay = ETHERNET.poll_delay(Instant::from_millis(current_time().as_millis() as i64));
    USB.start_kernel_timer(delay, Some(poll_ethernet));
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
        //trace!("pid {} added to core {}", id, affinity());
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
    /// フレームを `tf` に復元することでコンテキストスイッチを行う。poll_ethernet
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
            //trace!("[{}] no process", affinity());
            return None;
        }
        //trace!("sw_to_before.tf\n{:?}", &tf);
        let mut process = self.processes.remove(index).unwrap();
        process.state = State::Running;
        *tf = *process.context;
        let id = process.context.tpidr;
        //trace!("sw_to_after.tf\n{:?}", &tf);
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
                self.release_process_resources(tf);
                process.state = State::Dead;
                //trace!("[{}]: kill pid={}", affinity(), process.context.tpidr);
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

    /// カレントプロセスが保持するソケットなどのプロセスリソースをすべて解放する.
    fn release_process_resources(&mut self, tf: &mut TrapFrame) {
        // Lab 5 2.C
        match self.current_process(tf) {
            Some(index) => {
                let mut process = self.processes.remove(index).unwrap();
                for handle in process.sockets.iter_mut() {
                    ETHERNET.critical(|driver| {
                        driver.get_socket(*handle).close();
                        driver.release(*handle);
                        driver.prune();
                    });
                }
                ()
            }
            None => (),
        }
    }

    /// トラップフレームに保存されているtpidrに対応するプロセスを見つけつ.
    /// 検索に失敗したらパニック.
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
