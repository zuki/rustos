fn is_power2(value: usize) -> bool {
    if value == 0 {
        return false;
    }
    value & (value - 1) == 0
}
/// `addr` を `align` の最も近い倍数の下方向にアラインする.
///
/// 返される`usize`は常に <= `addr.`
///
/// # Panics
///
/// `align` は2のべき乗でない場合はパニック.
pub fn align_down(addr: usize, align: usize) -> usize {
    if !is_power2(align) {
        panic!("not a power of 2");
    }
    (addr  / align) * align
}

/// `addr` を `align` の最も近い倍数の上方向にアラインする.
///
/// 返される`usize`は常に >= `addr.`
///
/// # Panics
///
/// `align` は2のべき乗でない、または、アライメントでアドレスが
/// オーバーフローする場合はパニック.
pub fn align_up(addr: usize, align: usize) -> usize {
    if !is_power2(align) {
        panic!("not a power of 2");
    } else if addr + align > std::usize::MAX {
        panic!("aligning up overflows the address")
    }
    ((addr + align - 1) / align) * align
}
