const SETWAY_LEVEL_SHIFT: usize = 1;

const L1_DATA_CACHE_SETS: usize = 256;
const L1_DATA_CACHE_WAYS: usize = 2;
const L1_SETWAY_WAY_SHIFT: usize = 31;  // 32-Log2(L1_DATA_CACHE_WAYS)
const L1_SETWAY_SET_SHIFT: usize = 6;   // Log2(L1_DATA_CACHE_LINE_LENGTH)

const L2_CACHE_SETS: usize = 1024;
const L2_CACHE_WAYS: usize = 16;
const L2_SETWAY_WAY_SHIFT: usize = 28;  // 32-Log2(L2_CACHE_WAYS)
const L2_SETWAY_SET_SHIFT: usize = 6;   // Log2(L2_CACHE_LINE_LENGTH)

/// CPUを焼かないようにイベントを待機.
#[inline(always)]
pub fn wfe() {
    unsafe { asm!("wfe" :::: "volatile") };
}

/// CPUを焼かないように割り込みを待機.
#[inline(always)]
pub fn wfi() {
    unsafe { asm!("wfi" :::: "volatile") };
}

/// イベントをセット
#[inline(always)]
pub fn sev() {
    unsafe { asm!("sev" ::::"volatile") };
}

/// 最適化を防ぐためのNOOP.
#[inline(always)]
pub fn nop() {
    unsafe { asm!("nop" :::: "volatile") };
}

/// 下位レベルに移行
#[inline(always)]
pub fn eret() {
    unsafe { asm!("eret" :::: "volatile") };
}

/// 命令同期バリア
#[inline(always)]
pub fn isb() {
    unsafe { asm!("isb" ::: "memory": "volatile") };
}

/// 命令メモリバリア
#[inline(always)]
pub fn imb() {
    unsafe { asm!("isb" ::: "memory": "volatile") };
}

/// データ同期バリア
#[inline(always)]
pub fn dsb() {
    unsafe { asm!("dsb sy" ::: "memory" : "volatile") };
}

/// データメモリバリア
#[inline(always)]
pub fn dmb() {
    unsafe { asm!("dmb sy" ::: "memory" : "volatile") };
}

/// Enable (unmask) interrupts
#[inline(always)]
pub fn enable_irq_interrupt() {
    unsafe {
        asm!("msr DAIFClr, 0b0010"
         :
         :
         :
         : "volatile");
    }
}

/// Disable (mask) interrupt
#[inline(always)]
pub fn disable_irq_interrupt() {
    unsafe {
        asm!("msr DAIFSet, 0b0010"
         :
         :
         :
         : "volatile");
    }
}

/// Break with an immeidate
#[macro_export]
macro_rules! brk {
    ($num:tt) => {
        unsafe { asm!(concat!("brk ", stringify!($num)) :::: "volatile"); }
    }
}

/// Supervisor call with an immediate
#[macro_export]
macro_rules! svc {
    ($num:tt) => {
        unsafe { asm!(concat!("svc ", stringify!($num)) :::: "volatile"); }
    }
}

/// Enable (unmask) FIQ
#[inline(always)]
pub fn enable_fiq_interrupt() {
    unsafe {
        asm!("msr DAIFClr, 0b0001"
         :
         :
         :
         : "volatile");
    }
}

/// Disable (mask) FIQ
#[inline(always)]
pub fn disable_fiq_interrupt() {
    unsafe {
        asm!("msr DAIFSet, 0b0001"
         :
         :
         :
         : "volatile");
    }
}

pub fn get_interrupt_mask() -> u64 {
    unsafe {
        let mut mask: u64;
        asm!("mrs $0, DAIF"
         : "=r"(mask)
         :
         :
         : "volatile");
        mask
    }
}

pub fn set_interrupt_mask(mask: u64) {
    unsafe {
        asm!("msr DAIF, $0"
         :
         : "r"(mask)
         :
         : "volatile");
    }
}

/// データキャッシュをクリア
pub fn clear_data_cache() {
    // L1データキャッシュのクリア
    for nset in 0..L1_DATA_CACHE_SETS {
        for nway in 0..L1_DATA_CACHE_WAYS {
            let way_level = nway << L1_SETWAY_WAY_SHIFT
                    | nset << L1_SETWAY_SET_SHIFT
                    | 0 << SETWAY_LEVEL_SHIFT;
            unsafe { asm!("dc csw, $0" :: "r"(way_level) :: "volatile"); }
        }
    }
    // L2データキャッシュのクリア
    for nset in 0..L2_CACHE_SETS {
        for nway in 0..L2_CACHE_WAYS {
            let way_level = nway << L2_SETWAY_WAY_SHIFT
                    | nset << L2_SETWAY_SET_SHIFT
                    | 1 << SETWAY_LEVEL_SHIFT;
            unsafe { asm!("dc csw, $0" :: "r"(way_level) :: "volatile"); }
        }
    }
    dsb();
}
