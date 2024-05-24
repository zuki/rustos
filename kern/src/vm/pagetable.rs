use core::iter::Chain;
use core::ops::{Deref, DerefMut, Drop};
use core::slice::Iter;

use alloc::boxed::Box;
use alloc::fmt;
use core::alloc::{GlobalAlloc, Layout};

use crate::allocator;
use crate::param::*;
use crate::vm::{PhysicalAddr, VirtualAddr};
use crate::ALLOCATOR;
use crate::console::kprintln;

use aarch64::vmsa::*;
use shim::const_assert_size;
use pi::common::{IO_BASE, IO_BASE_END};

#[repr(C)]
pub struct Page([u8; PAGE_SIZE]);
const_assert_size!(Page, PAGE_SIZE);

impl Page {
    pub const SIZE: usize = PAGE_SIZE;
    pub const ALIGN: usize = PAGE_SIZE;

    fn layout() -> Layout {
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L2PageTable {
    pub entries: [RawL2Entry; 8192],
}
const_assert_size!(L2PageTable, PAGE_SIZE);

impl L2PageTable {
    /// 新規 `L2PageTable` を返す
    fn new() -> L2PageTable {
        Self {
            entries: [RawL2Entry::new(0) ; 8192],
        }
    }

    /// ページテーブルの `PhysicalAddr` を返す.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(&self.entries as *const RawL2Entry as u64)
    }
}

#[derive(Copy, Clone)]
pub struct L3Entry(RawL3Entry);

impl L3Entry {
    /// 新規 `L3Entry` を返す.
    fn new() -> L3Entry {
        L3Entry(RawL3Entry::new(0))
    }

    /// L3Entry が有効の場合、`true` を, そうでない場合は
    /// `false` を返す.
    fn is_valid(&self) -> bool {
        self.0.get_masked(RawL3Entry::VALID) == 1
    }

    /// L3Entryの `ADDR` フィールドを取り出し、それが有効であれば
    /// `PhysicalAddr` を、そうでなければ `None` を返す.
    fn get_page_addr(&self) -> Option<PhysicalAddr> {
        if self.is_valid() {
            Some(PhysicalAddr::from(self.0.get_value(RawL3Entry::ADDR)))
        } else {
            None
        }
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct L3PageTable {
    pub entries: [L3Entry; 8192],
}
const_assert_size!(L3PageTable, PAGE_SIZE);

impl L3PageTable {
    /// 新規 `L3PageTable` を返す.
    fn new() -> L3PageTable {
        Self {
            entries: [L3Entry::new(); 8192],
        }
    }

    /// ページテーブルの  `PhysicalAddr`を返す.
    pub fn as_ptr(&self) -> PhysicalAddr {
        PhysicalAddr::from(&self.entries as *const L3Entry as u64)
    }
}

#[repr(C)]
#[repr(align(65536))]
pub struct PageTable {
    pub l2: L2PageTable,
    pub l3: [L3PageTable; 3],
}

impl PageTable {
    /// `PageTable` を含む新規 `Box` を返す. L2PageTableの
    /// エントリは返す前に適切に初期化する必要がある.
    fn new(perm: u64) -> Box<PageTable> {
        let mut pt = Box::new(PageTable {
            l2: L2PageTable::new(),
            l3: [L3PageTable::new(), L3PageTable::new(), L3PageTable::new()],
        });
        for i in 0..3 {
            let addr = pt.l3[i].as_ptr();
            //kprintln!("l3[{}] addr = 0x{:08X}", i, addr.as_u64());
            let entry = &mut pt.l2.entries[i];
            entry.set_masked(addr.as_u64(), RawL2Entry::ADDR);
            entry.set_bit(RawL2Entry::AF);
            entry.set_value(EntrySh::ISh, RawL2Entry::SH);
            entry.set_value(perm, RawL2Entry::AP);
            entry.set_value(EntryAttr::Mem, RawL2Entry::ATTR);
            entry.set_value(EntryType::Table, RawL2Entry::TYPE);
            entry.set_bit(RawL2Entry::VALID);
        }
        //kprintln!("PT BASE_ADDR: 0x{:08X}", pt.l2.as_ptr().as_u64());
        pt
    }

    /// 指定の仮想アドレスから取り出した (L2index, L3index) を返す.
    /// このシステムでは1GBの仮想アドレスしかサポートしていないので
    /// L2index は2未満のはず。
    ///
    /// # パニック
    ///
    /// 仮想アドレスがページサイズに正しくアラインしていない場合はパニック.
    /// 取り出したL2indexがL3PageTableの数を超えていた場合はパニック.
    fn locate(va: VirtualAddr) -> (usize, usize) {
        let va = VirtualAddrEntry::new(va.as_u64());
        let l2_index = va.get_value(VirtualAddrEntry::L2INDEX);
        let l3_index = va.get_value(VirtualAddrEntry::L3INDEX);
        let pa = va.get_value(VirtualAddrEntry::PA);
        if l2_index > 2 {
            panic!("l2_index > 1: {}", l2_index);
        }
        if pa != 0 {
            panic!("va not aligned: {}", pa);
        }
        (l2_index as usize, l3_index as usize)

    }

    /// 指定の仮想アドレスが示すL3entryが有効な場合は `true` を、
    /// 総でない場合は `false` を返す.
    pub fn is_valid(&self, va: VirtualAddr) -> bool {
        let (l2_index, l3_index) = PageTable::locate(va);
        let entry = self.l3[l2_index].entries[l3_index];
        entry.is_valid()
    }

    /// 指定の仮想アドレスが示すL3entryが無効な場合は `true` を、
    /// 総でない場合は `false` を返す.
    pub fn is_invalid(&self, va: VirtualAddr) -> bool {
        !self.is_valid(va)
    }

    /// 指定されたRawL3Entry `entry` を仮想アドレスが示すL3Entryにセットする.
    pub fn set_entry(&mut self, va: VirtualAddr, entry: RawL3Entry) -> &mut Self {
        let (l2_index, l3_index) = PageTable::locate(va);
        self.l3[l2_index].entries[l3_index] = L3Entry(entry);
        self
    }

    /// このページテーブルのベースアドレスを返す. 返される `PhysicalAddr` 値は
    /// L2PageTableの開始アドレスを指している.
    pub fn get_baddr(&self) -> PhysicalAddr {
        self.l2.as_ptr()
    }
}

// FIXME: Implement `IntoIterator` for `&PageTable`.
impl<'a> IntoIterator for &'a PageTable {
    type Item = &'a L3Entry;
    type IntoIter = Chain<Iter<'a, L3Entry>, Iter<'a, L3Entry>>;

    fn into_iter(self) -> Self::IntoIter {
        self.l3[0].entries.iter().chain(self.l3[1].entries.iter())
    }
}

pub struct KernPageTable(Box<PageTable>);

impl KernPageTable {
    /// 新規 `KernPageTable` を返す. `KernPageTable` は `KERN_RW` 権限で
    /// 作成された `Pagetable` を持っている必要がある。
    ///
    /// RAM用の 0x00000000 から始まるARM物理アドレスとペリフェラル用の
    /// `IO_BASE` から `IO_BASE_END` の物理アドレス範囲のL3entry をセットする。
    /// 各 L3 エントリにはアドレス [47:16] だけでなく、下位の属性[10:0] にも
    /// 正しい値をセットする必要がある。詳細は `vmsa.rs` にある `RawL3Entry` の
    /// 定義を参照.
    pub fn new() -> KernPageTable {
        let mut pt = PageTable::new(EntryPerm::KERN_RW);
        let (start_addr, end_addr) = allocator::memory_map().unwrap();
        //kprintln!("memory_map: 0x{:08X} - 0x{:08X}", start_addr, end_addr);
        //kprintln!("IO_BASE   : 0x{:08X} - 0x{:08X}", IO_BASE, IO_BASE_END);
        for addr in (0..end_addr).step_by(PAGE_SIZE) {
            let va = VirtualAddr::from(addr);
            let mut entry = RawL3Entry::new(0);
            entry.set_masked(addr as u64, RawL3Entry::ADDR);
            entry.set_bit(RawL3Entry::AF);
            entry.set_value(EntrySh::ISh, RawL3Entry::SH);
            entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
            entry.set_value(EntryType::Table, RawL3Entry::TYPE);
            entry.set_bit(RawL3Entry::VALID);
            pt.set_entry(va, entry);
        }
        for addr in (IO_BASE..IO_BASE_END).step_by(PAGE_SIZE) {
            let va = VirtualAddr::from(addr);
            let mut entry = RawL3Entry::new(0);
            entry.set_masked(addr as u64, RawL3Entry::ADDR);
            entry.set_bit(RawL3Entry::AF);
            entry.set_value(EntrySh::OSh, RawL3Entry::SH);
            entry.set_value(EntryPerm::KERN_RW, RawL3Entry::AP);
            entry.set_value(EntryAttr::Dev, RawL3Entry::ATTR);
            entry.set_value(EntryType::Table, RawL3Entry::TYPE);
            entry.set_bit(RawL3Entry::VALID);
            pt.set_entry(va, entry);
        }
        //kprintln!("l2.entries[..2]");
        //kprintln!("  {:?}", pt.l2.entries[0]);
        //kprintln!("l3[0].entries[..10]");
        //for entry in &pt.l3[0].entries[..10] {
        //    kprintln!("  {:?}", entry.0);
        //}
        //kprintln!("  {:?}", pt.l2.entries[1]);
        //kprintln!("l3[1].entries[8180..]");
        //for entry in &pt.l3[1].entries[8180..] {
        //    kprintln!("  {:?}", entry.0);
        //}
        Self(pt)
    }
}

pub enum PagePerm {
    RW,
    RO,
    RWX,
}

pub struct UserPageTable(Box<PageTable>);

impl UserPageTable {
    /// `USER_RW` 権限の `PageTable` を含む新規 `UserPageTable` を返す.
    pub fn new() -> UserPageTable {
        Self(PageTable::new(EntryPerm::USER_RW))
    }

    /// ページを割り当て、指定の仮想アドレスを割り当てたページの物理
    /// アドレスに変換するL3エントリをセットする. 割り当てたページを
    /// 返す.
    ///
    /// # パニック
    /// 仮想アドレスが `USER_IMG_BASE` 未満の場合はパニック.
    /// 仮想アドレスがすでに割り当てられていた場合はパニック.
    /// アロケータがページの割当に失敗した場合はパニック.
    ///
    /// TODO. use Result<T> and make it failurable
    /// TODO. use perm properly
    pub fn alloc(&mut self, va: VirtualAddr, _perm: PagePerm) -> &mut [u8] {
        if va.as_usize() < USER_IMG_BASE {
            panic!("va < USER_IMG_BASE: 0x{:x}", va.as_u64());
        }
        let va_offset = va - VirtualAddr::from(USER_IMG_BASE);
        if self.0.is_valid(va_offset) {
            panic!("va already allocated: 0x{:x}", va.as_u64());
        }

        let addr = unsafe { ALLOCATOR.alloc(Page::layout()) as u64 };
        //kprintln!("allocated at 0x{:x}", addr);
        if addr == 0 {
            panic!("allocation failed");
        }
        let mut entry = RawL3Entry::new(0);
        entry.set_masked(addr, RawL3Entry::ADDR);
        entry.set_bit(RawL3Entry::AF);
        entry.set_value(EntrySh::ISh, RawL3Entry::SH);
        entry.set_value(EntryPerm::USER_RW, RawL3Entry::AP);
        entry.set_value(EntryAttr::Mem, RawL3Entry::ATTR);
        entry.set_value(PageType::Page, RawL3Entry::TYPE);
        entry.set_bit(RawL3Entry::VALID);

        //kprintln!("{:?}", &entry);
        self.0.set_entry(va_offset, entry);
        unsafe { core::slice::from_raw_parts_mut(addr as *mut u8, Page::SIZE) }
    }
}

impl Deref for KernPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Deref for UserPageTable {
    type Target = PageTable;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KernPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DerefMut for UserPageTable {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// FIXME: Implement `Drop` for `UserPageTable`.
impl Drop for UserPageTable {
    fn drop(&mut self) {
        for entry in self.0.into_iter() {
            if entry.is_valid() {
                let addr = entry.get_page_addr().unwrap();
                unsafe {
                    ALLOCATOR.dealloc(addr.as_u64() as *mut u8, Page::layout());
                }
            }
        }
    }
}

// FIXME: Implement `fmt::Debug` as you need.
impl fmt::Debug for KernPageTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "KernPageTable: ")?;
        writeln!(f, "  L2Table: 0x{:08X}", self.0.get_baddr().as_u64())?;
        for i in 0..3 {
            writeln!(f, "    [0]: {:?}", self.0.l2.entries[i])?;
        }
        writeln!(f, "  L3Table[0]: 0x{:08X}", self.0.l3[0].as_ptr().as_u64())?;
        writeln!(f, "    [0]: {:?}", self.0.l3[0].entries[0].0)?;
        writeln!(f, "    [1]: {:?}", self.0.l3[0].entries[1].0)?;
        writeln!(f, "  L3Table[1]: 0x{:08X}", self.0.l3[1].as_ptr().as_u64())?;
        writeln!(f, "    [8190]: {:?}", self.0.l3[1].entries[8190].0)?;
        writeln!(f, "    [8191]: {:?}", self.0.l3[1].entries[8191].0)?;
        writeln!(f, "  L3Table[2]: 0x{:08X}", self.0.l3[2].as_ptr().as_u64())?;
        writeln!(f, "    [0]: {:?}", self.0.l3[2].entries[0].0)?;
        writeln!(f, "    [1]: {:?}", self.0.l3[2].entries[1].0)?;
        Ok(())
    }
}

impl fmt::Debug for UserPageTable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "UserPageTable: ")?;
        writeln!(f, "  L2Table: 0x{:08X}", self.0.get_baddr().as_u64())?;
        writeln!(f, "    [0]: {:?}", self.0.l2.entries[0])?;
        writeln!(f, "  L3Table: 0x{:08X}", self.0.l3[0].as_ptr().as_u64())?;
        //writeln!(f, "    [0]: {:?}", self.0.l3[0].entries[0].0)?;

        for (i, entry) in self.0.into_iter().enumerate() {
            if entry.is_valid() {
                writeln!(f, "    [{}]: {:?}", i, entry.0)?;
            }
        }

        Ok(())
    }
}
