#![feature(new_uninit)]
use alloc::boxed::Box;
use shim::io;
use shim::path::Path;

use aarch64;

use crate::param::*;
use crate::process::{Stack, State};
use crate::traps::TrapFrame;
use crate::vm::*;
use kernel_api::{OsError, OsResult};

/// プロセスID型用のType alias.
pub type Id = u64;

/// プロセスの全状態を表す構造体.
#[derive(Debug)]
pub struct Process {
    /// 保存したプロセスのトラップフレーム.
    pub context: Box<TrapFrame>,
    /// プロセスのスタック用に使用するメモリ割り当て.
    pub stack: Stack,
    /// プロセスの仮想メモリを記述するページテーブル
    // pub vmap: Box<UserPageTable>,
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
    pub fn new() -> OsResult<Process> {
        let stack = Stack::new().ok_or(OsError::NoMemory)?;

        Ok(Process {
            context: Box::new(Default::default()),
            stack,
            state: State::Ready,
        })
    }

    /// Load a program stored in the given path by calling `do_load()` method.
    /// Set trapframe `context` corresponding to the its page table.
    /// `sp` - the address of stack top
    /// `elr` - the address of image base.
    /// `ttbr0` - the base address of kernel page table
    /// `ttbr1` - the base address of user page table
    /// `spsr` - `F`, `A`, `D` bit should be set.
    ///
    /// Returns Os Error if do_load fails.
    pub fn load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        use crate::VMM;

        let mut p = Process::do_load(pn)?;

        //FIXME: Set trapframe for the process.

        Ok(p)
    }

    /// Creates a process and open a file with given path.
    /// Allocates one page for stack with read/write permission, and N pages with read/write/execute
    /// permission to load file's contents.
    fn do_load<P: AsRef<Path>>(pn: P) -> OsResult<Process> {
        unimplemented!();
    }

    /// Returns the highest `VirtualAddr` that is supported by this system.
    pub fn get_max_va() -> VirtualAddr {
        unimplemented!();
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// memory space.
    pub fn get_image_base() -> VirtualAddr {
        unimplemented!();
    }

    /// Returns the `VirtualAddr` represents the base address of the user
    /// process's stack.
    pub fn get_stack_base() -> VirtualAddr {
        unimplemented!();
    }

    /// Returns the `VirtualAddr` represents the top of the user process's
    /// stack.
    pub fn get_stack_top() -> VirtualAddr {
        unimplemented!();
    }

    /// Returns `true` if this process is ready to be scheduled.
    ///
    /// This functions returns `true` only if one of the following holds:
    ///
    ///   * The state is currently `Ready`.
    ///
    ///   * An event being waited for has arrived.
    ///
    ///     If the process is currently waiting, the corresponding event
    ///     function is polled to determine if the event being waiting for has
    ///     occured. If it has, the state is switched to `Ready` and this
    ///     function returns `true`.
    ///
    /// Returns `false` in all other cases.
    pub fn is_ready(&mut self) -> bool {
        unimplemented!("Process::is_ready()")
    }
}
