use crate::console::kprintln;
use crate::mutex::Mutex;

use aarch64::*;

mod address;
mod pagetable;

pub use self::address::{PhysicalAddr, VirtualAddr};
pub use self::pagetable::*;
use crate::param::{KERNEL_MASK_BITS, USER_MASK_BITS};

/// カーネルページテーブルのスレッドセーフ（lockng) のラッパーe.
pub struct VMManager(Mutex<Option<KernPageTable>>);

impl VMManager {
    /// 初期化していない `VMManager` を返す.
    ///
    /// 仮想メモリマネージャは始めてのメモリ割り当ての前に
    /// `initialize()` と `setup()` を呼び出して初期化しなければ
    /// ならない。これを怠るとパニックになる。
    pub const fn uninitialized() -> Self {
        VMManager(Mutex::new(None))
    }

    /// 仮想メモリマネージャを初期化するr.
    /// callerはカーネル初期化中にこの関数を一度だけ
    /// 呼び出さなければならない。
    pub fn initialize(&self) {
        //kprintln!("ininitaiize VMM: call KernPageTabel::new");
        *self.0.lock() = Some(KernPageTable::new());
        self.setup();
    }

    /// 仮想メモリマネージャを設定するr.
    /// callerはこの関数を呼ぶ前にかならず `initialize()` を
    /// 呼び出していなければならない.
    /// MAIR_EL1, TCR_EL1, TTBR0_EL1, TTBR1_EL1 の各レジスタに適切な設定フラグをセットする。
    ///
    /// # パニック
    ///
    /// カレントシステムが64KB粒度のメモリ変換をサポートして
    /// いないとパニックになる。
    pub fn setup(&self) {
        let kern_page_table = self.0.lock();
        // カーネルページテーブルのデバッグ出力
        //kprintln!("{:?}", kern_page_table.as_ref().unwrap());
        let baddr = kern_page_table.as_ref().unwrap().get_baddr().as_u64();

        unsafe {
            // 64KB粒度をサポート
            assert!(ID_AA64MMFR0_EL1.get_value(ID_AA64MMFR0_EL1::TGran64) == 0);
            // ips = 0b0010 : 40ビット 1TB
            let ips = ID_AA64MMFR0_EL1.get_value(ID_AA64MMFR0_EL1::PARange);
            //kprintln!("ips: 0x{:X}", ips);
            // (ref. D7.2.70: Memory Attribute Indirection Register)
            MAIR_EL1.set(
                (0xFF <<  0) |// AttrIdx=0: normal, IWBWA, OWBWA, NTR
                (0x04 <<  8) |// AttrIdx=1: device, nGnRE (must be OSH too)
                (0x44 << 16), // AttrIdx=2: non cacheable
            );
            // (ref. D7.2.91: Translation Control Register)
            TCR_EL1.set(
                (0b00 << 37) |// TBI=0, no tagging
                (ips  << 32) |// IPS
                (0b11 << 30) |// TG1=64k
                (0b11 << 28) |// SH1=3 inner
                (0b01 << 26) |// ORGN1=1 write back
                (0b01 << 24) |// IRGN1=1 write back
                (0b0  << 23) |// EPD1 enables higher half
                ((USER_MASK_BITS as u64) << 16) | // T1SZ=34 (1GB)
                (0b01 << 14) |// TG0=64k
                (0b11 << 12) |// SH0=3 inner
                (0b01 << 10) |// ORGN0=1 write back
                (0b01 <<  8) |// IRGN0=1 write back
                (0b0  <<  7) |// EPD0 enables lower half
                ((KERNEL_MASK_BITS as u64) << 0), // T0SZ=32 (4GB)
            );
            isb();

            TTBR0_EL1.set(baddr);
            TTBR1_EL1.set(baddr);

            asm!("dsb ish");
            isb();
            //                                                           I            C M
            // SCTLR_EL1: 0x0000_0000_30D0_0800 = 0011_0000_1101_0000/0000_1000_0000_0000
            SCTLR_EL1.set(SCTLR_EL1.get() | SCTLR_EL1::I | SCTLR_EL1::C | SCTLR_EL1::M);
            asm!("dsb sy");
            isb();
        }
    }

    /// カーネルページテーブルのベースアドレスを `PhysicalAddrP` として返す.
    pub fn get_baddr(&self) -> PhysicalAddr {
        let kern_page_table = self.0.lock();
        PhysicalAddr::from(kern_page_table.as_ref().unwrap().get_baddr().as_u64())
    }
}
