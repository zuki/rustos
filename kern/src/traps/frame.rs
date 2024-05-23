use core::fmt;

#[repr(C)]
#[derive(Default, Copy, Clone)]
pub struct TrapFrame {
    pub elr: u64,
    pub spsr: u64,
    pub sp: u64,
    pub tpidr: u64,
    pub ttbr0: u64,
    pub ttbr1: u64,
    pub qn: [u128; 32],
    pub xn: [u64; 31],
    pub zero: u64,
}

impl fmt::Debug for TrapFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "  ELR   : 0x{:08X}", self.elr)?;
        writeln!(f, "  SPSR  : 0x{:08X}", self.spsr)?;
        writeln!(f, "  SP    : 0x{:08X}", self.sp)?;
        writeln!(f, "  TPIDR : {}", self.tpidr)?;
        writeln!(f, "  TTBR0 : 0x{:08X}", self.ttbr0)?;
        writeln!(f, "  TTBR1 : 0x{:08X}", self.ttbr1)?;
        writeln!(f, "  x0    : 0x{:08X}", self.xn[0])?;
        writeln!(f, "  x1    : 0x{:08X}", self.xn[1])?;
        writeln!(f, "  x7    : 0x{:08X}", self.xn[7])?;
        writeln!(f, "  x30   : 0x{:08X}", self.xn[30])?;
        Ok(())
    }
}
