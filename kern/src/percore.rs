use core::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use crate::param::NCORES;
use crate::traps::irq::LocalIrq;

/// per-coreデータを追跡するための構造体.
#[repr(align(512))]
pub struct PerCore {
    /// このコアで保持しているロックの数
    preemption: AtomicI64,
    /// このコア用のMMUは初期化されているか?
    mmu_ready: AtomicBool,
    /// ローカルIRQハンドラレジストリ
    irq: LocalIrq,
}

static PER_CORE_DATA: [PerCore; NCORES] = [
    PerCore {
        preemption: AtomicI64::new(0),
        mmu_ready: AtomicBool::new(false),
        irq: LocalIrq::new(),
    },
    PerCore {
        preemption: AtomicI64::new(0),
        mmu_ready: AtomicBool::new(false),
        irq: LocalIrq::new(),
    },
    PerCore {
        preemption: AtomicI64::new(0),
        mmu_ready: AtomicBool::new(false),
        irq: LocalIrq::new(),
    },
    PerCore {
        preemption: AtomicI64::new(0),
        mmu_ready: AtomicBool::new(false),
        irq: LocalIrq::new(),
    },
];

/// このコアの現在のプリエンプションカウンタを返す.
pub fn get_preemptive_counter() -> i64 {
    let cpu = aarch64::affinity();
    PER_CORE_DATA[cpu].preemption.load(Ordering::Relaxed)
}

/// このコアのプリエンプションカウンタをインクルメントして現在のコア番号を返す.
pub fn getcpu() -> usize {
    let cpu = aarch64::affinity();
    PER_CORE_DATA[cpu]
        .preemption
        .fetch_add(1, Ordering::Relaxed);
    cpu
}

/// このコアのプリエンプションカウンタをデクリメントする。この関数は
/// `cpu`パラメタがカレントコア番号に一致することをアサートする.
pub fn putcpu(cpu: usize) {
    assert!(aarch64::affinity() == cpu, "Incorrect putcpu()");
    let cnt = PER_CORE_DATA[cpu]
        .preemption
        .fetch_sub(1, Ordering::Relaxed);
    assert!(cnt > 0, "Preemption count goes to negative!")
}

/// カレントコアでMMUが初期化されている場合、trueを返す.
pub fn is_mmu_ready() -> bool {
    let cpu = aarch64::affinity();
    PER_CORE_DATA[cpu].mmu_ready.load(Ordering::Relaxed)
}

/// カレントコアのMMU-readyフラグをセットする.
pub unsafe fn set_mmu_ready() {
    let cpu = aarch64::affinity();
    PER_CORE_DATA[cpu].mmu_ready.store(true, Ordering::Relaxed);
}

/// カレントコアのローカルIRQハンドラレジストリへの参照を返す.
pub fn local_irq() -> &'static LocalIrq {
    let cpu = aarch64::affinity();
    &PER_CORE_DATA[cpu].irq
}
