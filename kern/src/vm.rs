mod address;
mod pagetable;

pub use self::address::{PhysicalAddr, VirtualAddr};
pub use self::pagetable::*;

use aarch64::*;
use pi::common::NCORES;
use core::sync::atomic::{AtomicUsize, Ordering};

use crate::mutex::Mutex;
use crate::param::{KERNEL_MASK_BITS, USER_MASK_BITS};
use crate::percore::{is_mmu_ready, set_mmu_ready};

pub struct VMManager {
    /// カーネルページテーブル
    kern_pt: Mutex<Option<KernPageTable>>,
    /// カーネルページテーブルのベースアドレス
    kern_pt_addr: AtomicUsize,
    /// MMU初期化済みのコアの数
    ready_core_cnt: AtomicUsize,
}

impl VMManager {
    /// 初期化していない `VMManager` を返す.
    ///
    /// 仮想メモリマネージャは始めてのメモリ割り当ての前に
    /// `initialize()` と `setup()` を呼び出して初期化しなければ
    /// ならない。これを怠るとパニックになる。
    pub const fn uninitialized() -> Self {
        VMManager {
            kern_pt: Mutex::new(None),
            kern_pt_addr: AtomicUsize::new(0),
            ready_core_cnt: AtomicUsize::new(0),
        }
    }

    /// 仮想メモリマネージャを初期化する.
    /// callerはカーネル初期化中にこの関数を一度だけ
    /// 呼び出さなければならない。
    pub unsafe fn initialize(&self) {
        //kprintln!("ininitaiize VMM: call KernPageTabel::new");
        let mut kern_pt = KernPageTable::new();
        let baddr_kern_pt = kern_pt.get_baddr();

        if self.kern_pt.lock().replace(kern_pt).is_some() {
            panic!("VMManager initialize called twice");
        }
        self.kern_pt_addr.store(baddr_kern_pt.as_usize(), Ordering::Relaxed);
    }

    /// 仮想メモリマネージャを設定する.
    /// callerはこの関数を呼ぶ前にかならず `initialize()` を
    /// 呼び出していなければならない.
    /// MAIR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1 の各レジスタに適切な
    /// 設定フラグをセットする。
    ///
    /// # パニック
    ///
    /// カレントシステムが64KBのメモリ翻訳粒度サイズをサポートして
    /// いない場合はパニックになる.
    pub unsafe fn setup(&self) {
        assert!(ID_AA64MMFR0_EL1.get_value(ID_AA64MMFR0_EL1::TGran64) == 0);

        let ips = ID_AA64MMFR0_EL1.get_value(ID_AA64MMFR0_EL1::PARange);

        // (ref. D7.2.70: Memory Attribute Indirection Register)
        MAIR_EL1.set(
            (0xFF <<  0) |// AttrIdx=0: normal, IWBWA, OWBWA, NTR
            (0x04 <<  8) |// AttrIdx=1: device, nGnRE (must be OSH too)
            (0x44 << 16), // AttrIdx=2: non cacheable
        );

        // (ref. D7.2.91: Translation Control Register)
        TCR_EL1.set(
            (0b00 << 37) | // TBI=0, no tagging
            (ips  << 32) | // IPS
            (0b11 << 30) | // TG1=64k
            (0b11 << 28) | // SH1=3 inner
            (0b01 << 26) | // ORGN1=1 write back
            (0b01 << 24) | // IRGN1=1 write back
            (0b0  << 23) | // EPD1 enables higher half
            ((USER_MASK_BITS as u64) << 16) | // T1SZ=34 (1GB)
            (0b01 << 14) | // TG0=64k
            (0b11 << 12) | // SH0=3 inner
            (0b01 << 10) | // ORGN0=1 write back
            (0b01 <<  8) | // IRGN0=1 write back
            (0b0  <<  7) | // EPD0 enables lower half
            ((KERNEL_MASK_BITS as u64) << 0), // T0SZ=31 (8GB)
        );
        isb();

        let baddr = self.kern_pt_addr.load(Ordering::Relaxed);

        TTBR0_EL1.set(baddr as u64);
        TTBR1_EL1.set(baddr as u64);

        asm!("dsb ish");
        isb();

        //                                                           I            C M
        // SCTLR_EL1: 0x0000_0000_30D0_0800 = 0011_0000_1101_0000/0000_1000_0000_0000
        SCTLR_EL1.set(SCTLR_EL1.get() | SCTLR_EL1::I | SCTLR_EL1::C | SCTLR_EL1::M);
        asm!("dsb sy");
        isb();
        // このコアでMMUの初期化がすんだことを記録する
        set_mmu_ready();
    }

    /// カレントコアのMMUを設定する.
    /// すべてのコアが各自のMMUを初期化するまで待機する.
    pub fn wait(&self) {
        assert!(!is_mmu_ready());

        unsafe {
            self.setup();
        }

        //info!("MMU is ready for core-{}/@sp={:016x}", affinity(), SP.get());
        
        // Lab 5 1.B
        // MMU初期化済みのコア数をインクルメント
        self.ready_core_cnt.fetch_add(1, Ordering::AcqRel);
        // すべてのコアがMMUの初期化を終えるのを待機する
        while self.ready_core_cnt.load(Ordering::Acquire) != NCORES {}
    }

    /// カーネルページテーブルのベースアドレスを `PhysicalAddrP` として返す.
    pub fn get_baddr(&self) -> PhysicalAddr {
        PhysicalAddr::from(self.kern_pt_addr.load(Ordering::Relaxed))
    }
}

