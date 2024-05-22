#![feature(new_uninit)]
use alloc::boxed::Box;
use core::mem;
use shim::io;
use shim::path::Path;
use fat32::traits::{Entry, File, FileSystem};

use aarch64::*;

use crate::{param::*, FILESYSTEM};
use crate::process::{Stack, State};
use crate::traps::TrapFrame;
use crate::vm::*;
use kernel_api::{OsError, OsResult};
use crate::console::kprintln;
use crate::allocator::util::align_down;
use shim::io::{Read, Seek};

/// プロセスID型用のType alias.
pub type Id = u64;

/// プロセスの全状態を表す構造体.
#[derive(Debug)]
pub struct Process {
    /// 保存したプロセスのトラップフレーム.
    pub context: Box<TrapFrame>,
    // プロセスのスタック用に使用するメモリ割り当て.
    // スタックはページに割り当て（Stack構造体ではない）、tfで管理するので不要
    // pub stack: Stack,
    /// プロセスの仮想メモリを記述するページテーブル
    pub vmap: Box<UserPageTable>,
    /// プロセスのスケジューリング状態.
    pub state: State,
}

impl Process {
    /// ゼロ詰めの `TrapFrame` (デフォルト)、デフォルトサイズの
    /// ゼロ詰めのスタック、`Ready` 状態を持つ新しいプロセスを作成する.
    ///
    /// プロセスを開始するのに十分なメモリを確保できなかった場合は
    /// `OsError::NoMemory` を返す。そうでない場合は、新しい `Process`
    /// の `Some` を返す。
/*
    pub fn new() -> OsResult<Process> {
        // スタックはpagetableで割り当てる
        let stack = Stack::new().ok_or(OsError::NoMemory)?;
        //kprintln!("Procwss::new: call UserPageTable::new");
        Ok(Process {
            context: Box::new(TrapFrame::default()),
            //stack,
            vmap: Box::new(UserPageTable::new()),
            state: State::Ready,
        })
    }
*/

    /// `do_load()`メソッドを呼び出すことにより指定のパスに
    /// 格納されているプログラムをロードする.
    /// そのページテーブルに対応してトラップフレームの`context`をセットする.
    /// `sp` - スタックの先頭アドレス
    /// `elr` - イメージのベースアドレス.
    /// `ttbr0` - カーネルページテーブルのベースアドレス
    /// `ttbr1` - ユーザページテーブルのベースアドレス
    /// `spsr` - `F`, `A`, `D` ビットをセットする必要がある.
    ///
    /// do_load が失敗した場合は OSError を返す.
    pub fn load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        use crate::VMM;

        let mut p = Process::do_load(pn)?;

        //FIXME: Set trapframe for the process.
        let mut tf = &mut p.context;
        tf.elr   = Process::get_image_base().as_u64();
        tf.spsr  = (SPSR_EL1::M & 0b0000) | SPSR_EL1::F | SPSR_EL1::A | SPSR_EL1::D;
        tf.sp    = Process::get_stack_top().as_u64();
        tf.ttbr0 = crate::VMM.get_baddr().as_u64();
        tf.ttbr1 = p.vmap.get_baddr().as_u64();

        Ok(p)
    }

    /// プロセスを作成して、指定されたパスのファイルをオープンする.
    /// スタック用にread/write権限のページを1ページ割り当て、ファイル
    /// コンテンツのロード用にread/write/execute権限のページをNページ
    /// 割り当てる.
    fn do_load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        // 1. UserPageTableを作成
        let mut vmap = Box::new(UserPageTable::new());
        // 2. スタックを作成
        let mut stack = vmap.alloc(Process::get_stack_base(), PagePerm::RW);
        // 2.1 スタックを0クリア
        for byte in stack.iter_mut() {
            *byte = 0;
        }
        // 3. ファイルをオープン
        let entry = FILESYSTEM.open(pn)?;
        let mut file = entry.into_file().ok_or(OsError::NoEntry)?;
        // 4. ファイルを読み込む
        let mut addr = Process::get_image_base();
        let size = file.size() as usize;
        for i in 0..size / PAGE_SIZE {
            let mut page = vmap.alloc(addr, PagePerm::RWX);
            file.read_exact(&mut page)?;
            addr += VirtualAddr::from(PAGE_SIZE);
        }
        if size % PAGE_SIZE != 0 {
            let mut page = vmap.alloc(addr, PagePerm::RWX);
            file.read_exact(&mut page[..size % PAGE_SIZE])?;
        }
        Ok(Process {
            context: Box::new(TrapFrame::default()),
            vmap,
            state: State::Ready,
        })

    }

    /// このシステムにサポートされている最高位の `VirtualAddr` アドレスを返す.
    pub fn get_max_va() -> VirtualAddr {
        VirtualAddr::from(USER_MAX_VM)
    }

    /// ユーザのベーズアドレスを表す `VirtualAddr` を返す.
    pub fn get_image_base() -> VirtualAddr {
        VirtualAddr::from(USER_IMG_BASE)
    }

    /// ユーザプロセスのスタックのベースアドレスを表す `VirtualAddr` を返す.
    pub fn get_stack_base() -> VirtualAddr {
        VirtualAddr::from(USER_STACK_BASE)
    }

    /// ユーザプロセスのスタックの先頭を表す `VirtualAddr` を返す.
    pub fn get_stack_top() -> VirtualAddr {
        VirtualAddr::from(align_down(USER_MAX_VM, 16))
    }

    /// このプロセスがスケジュールされる準備ができている場合は
    /// `true` を返す。
    ///
    /// この関数は次のどちらかに該当する場合のみ `true` を返す。
    ///
    ///   * 現在の状態が `Ready`.
    ///
    ///   * 待機していたイベントが届いた。
    ///
    ///     プロセスが現在待機中の場合、対応するイベント関数が
    ///     ポーリングされ、待機中のイベントが発生したか否かを
    ///     判断する。発生していた場合は状態を `Ready` に切り替え、
    ///     この関数は `true` を返す。
    ///
    /// それ以外のすべての場合は `false` を返す。
    pub fn is_ready(&mut self) -> bool {
        let mut state = mem::replace(&mut self.state, State::Ready);
        match state {
            State::Ready => true,
            State::Waiting(ref mut event_poll_fn) => {
                if event_poll_fn(self) {
                    true
                } else {
                    self.state = state;
                    false
                }
            }
            _ => {
                self.state = state;
                false
            }
        }
    }
}
