/// I/Oぺりジェラルがマッピングされるアドレス.
pub const IO_BASE: usize = 0x3F000000;
pub const IO_BASE_END: usize = 0x40000000 + 0x20000000;

/// `GPIO` レジスタのベースアドレス
pub const GPIO_BASE: usize = IO_BASE + 0x200000;

/// Rpi3のコアの数
pub const NCORES: usize = 4;

/// 各コアがスピニングしているベース物理アドレス
pub const SPINNING_BASE: *mut usize = 0xd8 as *mut usize;

/// 渡された `ident` に対してバリアントを含まない `pub enum` を
/// 生成する.
pub macro states($($name:ident),*) {
    $(
        /// 可能な状態.
        #[doc(hidden)]
        pub enum $name {  }
    )*
}
