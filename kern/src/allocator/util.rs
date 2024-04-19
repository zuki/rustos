/// `addr` を `align` の最も近い倍数の下方向にアラインする.
///
/// 返される`usize`は常に <= `addr.`
///
/// # Panics
///
/// `align` は2のべき乗でない場合はパニック.
pub fn align_down(addr: usize, align: usize) -> usize {
    if align.count_ones() != 1 {
        panic!("not a power of 2");
    }
    addr & !(align - 1)
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
    if align.count_ones() != 1 {
        panic!("not a power of 2");
    }

    let aligned = addr & !(align - 1);
    if aligned < addr {
        if aligned + align <= core::usize::MAX {
            return aligned + align;
        } else {
            panic!("aligning up overflows the address");
        }
    }
    aligned
}
