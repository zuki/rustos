use aarch64::*;

use core::mem::zeroed;
use core::ptr::write_volatile;

mod oom;
mod panic;

use crate::kmain;
use crate::param::*;
use crate::VMM;
use crate::SCHEDULER;
use pi::common::SPINNING_BASE;

global_asm!(include_str!("init/vectors.s"));

//
// 大前提 (チェックすること):
//   _start1/2(), _kinit1/2(), switch_to_el1/2() は
//     スタックを使ってあいけないshould NOT use stack!
//   例えば, #[no_stack] が便利だろう ..
//
// そのため、debugビルドはサポートしていない!
//

/// コア0のカーネルエントリポイント
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    if MPIDR_EL1.get_value(MPIDR_EL1::Aff0) == 0 {
        SP.set(KERN_STACK_BASE);
        kinit()
    }
    unreachable!()
}

unsafe fn zeros_bss() {
    extern "C" {
        static mut __bss_beg: u64;
        static mut __bss_end: u64;
    }

    let mut iter: *mut u64 = &mut __bss_beg;
    let end: *mut u64 = &mut __bss_end;

    while iter < end {
        write_volatile(iter, zeroed());
        iter = iter.add(1);
    }
}

#[no_mangle]
unsafe fn switch_to_el2() {
    if current_el() == 3 {
        // ソース制御レジスタ (SCR_EL3) の設定 (D13.2.10)
        SCR_EL3.set(SCR_EL3::NS | SCR_EL3::SMD | SCR_EL3::HCE | SCR_EL3::RW | SCR_EL3::RES1);

        // 保存プログラムステータスレジスタ (SPSR_EL3) の設定 (C5.2.19)
        SPSR_EL3
            .set((SPSR_EL3::M & 0b1001) | SPSR_EL3::F | SPSR_EL3::I | SPSR_EL3::A | SPSR_EL3::D);

        // 自分指針にeret, これにより current_el() == 2 となる.
        ELR_EL3.set(switch_to_el2 as u64);
        asm::eret();
    }
}

#[no_mangle]
unsafe fn switch_to_el1() {
    extern "C" {
        static mut vectors: u64;
    }

    if current_el() == 2 {
        // EL1のスタックポインタを設定
        SP_EL1.set(SP.get() as u64);

        // EL1/EL0 でCNTPを有効化 (ref: D7.5.2, D7.5.13)
        // 注: これは実際にはカウンタストリームを有効にしない.
        CNTHCTL_EL2.set(CNTHCTL_EL2.get() | CNTHCTL_EL2::EL0VCTEN | CNTHCTL_EL2::EL0PCTEN);
        CNTVOFF_EL2.set(0);

        // EL1でAArch64を有効化 (A53: 4.3.36)
        HCR_EL2.set(HCR_EL2::RW | HCR_EL2::RES1);

        // 浮動小数点数とSVE (SIMD) を有効化 (A53: 4.3.38, 4.3.34)
        CPTR_EL2.set(0);
        CPACR_EL1.set(CPACR_EL1.get() | (0b11 << 20));

        // SCTRLを既知の状態に設定 (A53: 4.3.30)
        SCTLR_EL1.set(SCTLR_EL1::RES1);

        // 例外ハンドラを設定
        // `vectors`のアドレスを適切なレジスタにロードする (guide: 10.4)
        VBAR_EL1.set(&vectors as *const u64 as u64);

        // 例外レベルをEL1に変更する (ref: C5.2.19)
        SPSR_EL2.set(
            (SPSR_EL2::M & 0b0101) // EL1h
            | SPSR_EL2::F
            | SPSR_EL2::I
            | SPSR_EL2::D
            | SPSR_EL2::A,
        );

        // 自分自身にeretする。これにより current_el() == 1 となる
        ELR_EL2.set(switch_to_el1 as u64);
        asm::eret();
    }
}

#[no_mangle]
unsafe fn kinit() -> ! {
    zeros_bss();
    switch_to_el2();
    switch_to_el1();
    kmain();
}

/// コア 1, 2, 3 のカーネルエントリポイント
#[no_mangle]
pub unsafe extern "C" fn start2() -> ! {
    // Lab 5 1.A
    let core = MPIDR_EL1.get_value(MPIDR_EL1::Aff0);
    let stack =  KERN_STACK_BASE - KERN_STACK_SIZE * core as usize;
    asm!("mov sp, $0"
         :: "r"(stack) :: "volatile");
    kinit2();
}

unsafe fn kinit2() -> ! {
    switch_to_el2();
    switch_to_el1();
    kmain2()
}

unsafe fn kmain2() -> ! {
    // Lab 5 1.A
    let core = MPIDR_EL1.get_value(MPIDR_EL1::Aff0);
    let spinning = SPINNING_BASE.add(core as usize);
    spinning.write_volatile(0);
    VMM.wait();
    info!("core {} started", core);

    SCHEDULER.start()
}

/// `init::start2` のアドレスを各自のスピニングアドレスに
/// 書き込むことによりappコアを起床させ、`sev()`でイベントを
/// 送信する.
pub unsafe fn initialize_app_cores() {
    // Lab 5 1.A
    for core in 1..NCORES {
        let spinning = SPINNING_BASE.add(core);
        spinning.write_volatile(start2 as usize);
    }

    asm::sev();

    for core in 1..NCORES {
        let spinning = SPINNING_BASE.add(core);
        while spinning.read_volatile() != 0 {}
    }
}
