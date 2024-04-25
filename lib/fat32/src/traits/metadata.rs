/// タイムスタンプ（年、月、日、時、分、秒）用のトレイト.
pub trait Timestamp: Copy + Clone + Sized {
    /// カレンダー年.
    ///
    /// 年はオフセットではない。2009は2009である。
    fn year(&self) -> usize;

    /// カレンダー月、1月の1から始まる。常に range [1, 12] にある。
    ///
    /// 1月は1, 2月は2, ..., 12月は12.
    fn month(&self) -> u8;

    /// カレンダー日。1から始まる。常に range [1, 31] にある。
    fn day(&self) -> u8;

    /// 24時間の時。常に range [0, 24] にある。
    fn hour(&self) -> u8;

    /// 分。常に range [0, 60] にある。
    fn minute(&self) -> u8;

    /// 秒。常に range [0, 60) にある。
    fn second(&self) -> u8;
}

/// ディレクトリエントリメタデータ用のトレイト.
pub trait Metadata: Sized {
    /// ある時点に買王する型.
    type Timestamp: Timestamp;

    /// 関連エントリが読み込み専用か否か.
    fn read_only(&self) -> bool;

    /// エントリをディレクトリ走査から「隠す」べきか否か.
    fn hidden(&self) -> bool;

    /// エントリが作成されたときのタイムスタンプ.
    fn created(&self) -> Self::Timestamp;

    /// エントリが最後にアクセスされたときのタイムスタンプ.
    fn accessed(&self) -> Self::Timestamp;

    /// エントリが最後に変更されたときのタイムスタンプ.
    fn modified(&self) -> Self::Timestamp;
}
