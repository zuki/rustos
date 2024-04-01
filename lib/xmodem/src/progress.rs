/// 送受信の進捗状況を表す列挙型.
///
/// この型の値は[`Xmodem::transmit_with_progress()`],
/// [`Xmodem::receive_with_progress()`], [`Xmodem::new_with_progress()`]
/// などのメソッドに渡される進捗コールバックに渡される。
/// これは進捗インジケータやデバッグ目的で使用することを意図している。
#[derive(Debug, Copy, Clone)]
pub enum Progress {
    /// NAKを送信する受信者を待っている.
    Waiting,
    /// ダウンロード/アップロードが開始された.
    Started,
    /// パケット `.0` が送信/受信された.
    Packet(u8),
    NAK,
    Unknown,
}

/// 進捗コールバックの型.
pub type ProgressFn = fn(Progress);

/// Noop進捗コールバック.
pub fn noop(_: Progress) {  }
