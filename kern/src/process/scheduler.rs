use alloc::boxed::Box;
use alloc::collections::vec_deque::VecDeque;
use core::fmt;

use aarch64::*;

use crate::mutex::Mutex;
use crate::param::{PAGE_MASK, PAGE_SIZE, TICK, USER_IMG_BASE};
use crate::process::{Id, Process, State};
use crate::traps::{irq, TrapFrame};
use crate::VMM;
use crate::IRQ;
use crate::SCHEDULER;
use crate::console::{kprint, kprintln};
use pi::timer;
use pi::interrupt::{Interrupt, Controller};

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
            //aarch64::wfe();
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
        // タイマー割り込みハンドラの設定
        IRQ.register(
            Interrupt::Timer1,
            Box::new(|tf| {
                timer::tick_in(TICK);
                let old_id = tf.tpidr;
                let id = SCHEDULER.switch(State::Ready, tf);
                kprintln!("TICK, switch from {} to {}", old_id, id);
            }),
        );
        // タイマー割り込みの有効化
        timer::tick_in(TICK);
        let mut controller = Controller::new();
        controller.enable(Interrupt::Timer1);

        let mut tf = Box::new(TrapFrame::default());
        self.critical(|scheduler| scheduler.switch_to(&mut tf));

        //kprintln!("tf\n{:?}", tf);
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
        //kprintln!("eret start");
        eret();
        loop {};
    }



    /// スケジューラを初期化してユーザ空間のプロセスをスケジューラに追加する.
    pub unsafe fn initialize(&self) {
    /*
        let mut process1 = Process::new().expect("new process");
        let mut tf1 = &mut process1.context;
        // 例外からの戻り先はstart_shell()関数
        tf1.elr = start_shell as *const u64 as u64;
        // SPSR_EL1をセット。IRQはアンマスク
        tf1.spsr = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        // SP_EL0はこのプロセスのスタックのtop
        tf1.sp = process1.stack.top().as_u64();

        let mut process2 = Process::new().expect("new process");
        let mut tf2 = &mut process2.context;
        tf2.elr = test_proc_2 as *const u64 as u64;
        tf2.spsr = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        tf2.sp = process2.stack.top().as_u64();

        let mut process3 = Process::new().expect("new process");
        let mut tf3 = &mut process3.context;
        tf3.elr = test_proc_3 as *const u64 as u64;
        tf3.spsr = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        tf3.sp = process3.stack.top().as_u64();
*/

        let mut process1 = Process::new().expect("new process");
        //kprint!("{:?}", &process1.vmap);
        let mut tf = &mut process1.context;
        tf.elr = USER_IMG_BASE as *const u64 as u64;
        tf.spsr = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        tf.sp = process1.stack.top().as_u64();
        tf.ttbr0 = crate::VMM.get_baddr().as_u64();
        tf.ttbr1 = process1.vmap.get_baddr().as_u64();
        //kprintln!("proc1.tf:\n{:?}", tf);
        //kprintln!("sp_bottom: 0x{:X}", process1.stack.bottom().as_u64());
        self.test_phase_3(&mut process1);

        let mut process2 = Process::new().expect("new process");
        //kprint!("{:?}", &process2.vmap);
        let mut tf = &mut process2.context;
        tf.elr = USER_IMG_BASE as *const u64 as u64;
        tf.spsr = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        tf.sp = process2.stack.top().as_u64();
        tf.ttbr0 = crate::VMM.get_baddr().as_u64();
        tf.ttbr1 = process2.vmap.get_baddr().as_u64();
        //kprintln!("proc2.tf:\n{:?}", tf);
        self.test_phase_3(&mut process2);

        let mut scheduler = Scheduler::new();
        scheduler.add(process1);
        scheduler.add(process2);
        //scheduler.add(process3);
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
        kprint!("{:?}", &proc.vmap);
    }

}

#[derive(Debug)]
pub struct Scheduler {
    /// プロセスキュー
    processes: VecDeque<Process>,
    // プロセスID付番用
    last_id: Option<Id>,
}

impl Scheduler {
    /// 空のキューを持つ新しい `Scheduler` を返す.
    fn new() -> Scheduler {
        Scheduler {
            processes: VecDeque::<Process>::new(),
            last_id: None,
        }
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

pub extern "C" fn start_shell() -> ! {
    use crate::shell;

    //unsafe { asm!("brk 1" :::: "volatile"); }
    //unsafe { asm!("brk 2" :::: "volatile"); }
    //shell::shell("user0> ");
    //unsafe { asm!("brk 3" :::: "volatile"); }
    loop {
        shell::shell("user1> ");
    }
}

pub extern "C" fn test_proc_2() -> ! {
    use crate::shell;
    loop {
        shell::shell("user2> ");
    }
}

pub extern "C" fn test_proc_3() -> ! {
    use crate::shell;
    loop {
        shell::shell("user3> ");
    }
}
