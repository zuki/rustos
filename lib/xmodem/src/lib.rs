#![cfg_attr(feature = "no_std", no_std)]

#![feature(decl_macro)]

use shim::io;
use shim::ioerr;

#[cfg(test)] mod tests;
mod read_ext;
mod progress;

pub use progress::{Progress, ProgressFn};

use read_ext::ReadExt;

const SOH: u8 = 0x01;
const EOT: u8 = 0x04;
const ACK: u8 = 0x06;
const NAK: u8 = 0x15;
const CAN: u8 = 0x18;

/// XMODEMプロトコルの実装.
pub struct Xmodem<R> {
    packet: u8,
    started: bool,
    inner: R,
    progress: ProgressFn
}

impl Xmodem<()> {
    /// XMODEMプロトコルを使用して`data`を受信側`to` に送信する.
    /// `data`の全長が128バイトの倍数でない場合は0でパディングして
    /// 受信側に送信する。
    ///
    /// パディングゼロを除いた`to`に書き込まれたバイト数を返す。
    #[inline]
    pub fn transmit<R, W>(data: R, to: W) -> io::Result<usize>
        where W: io::Read + io::Write, R: io::Read
    {
        Xmodem::transmit_with_progress(data, to, progress::noop)
    }

    /// XMODEMプロトコルを使用して`data`を受信側`to` に送信する.
    /// `data`の全長が128バイトの倍数でない場合は0でパディングして
    /// 受信側に送信する。
    ///
    /// 関数 `f` は、送信の進行状況を示すコールバックとして使用される。
    /// 詳細は [`Progress`] enumを参照のこと。
    ///
    /// パディングゼロを除いた`to`に書き込まれたバイト数を返す。
    pub fn transmit_with_progress<R, W>(mut data: R, to: W, f: ProgressFn) -> io::Result<usize>
        where W: io::Read + io::Write, R: io::Read
    {
        let mut transmitter = Xmodem::new_with_progress(to, f);
        let mut packet = [0u8; 128];
        let mut written = 0;
        'next_packet: loop {
            // dataから最大128バイト取り出してpacketにいれる。
            let n = data.read_max(&mut packet)?;
            // パケットが128に満たない場合は0を詰める
            packet[n..].iter_mut().for_each(|b| *b = 0);
            // dataが殻になったら転送終了
            if n == 0 {
                transmitter.write_packet(&[])?;
                return Ok(written);
            }
            // パケット転送を最大10回試行
            for _ in 0..10 {
                match transmitter.write_packet(&packet) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(_) => {
                        written += n;
                        continue 'next_packet;
                    }
                }
            }

            return ioerr!(BrokenPipe, "bad transmit");
        }
    }

    /// XMODEMプロトコルを使用して `from` から `data` を受け取り、
    /// `into` に書き込む. `from` から読み込んだバイト数（128の倍数）を返す。
    #[inline]
    pub fn receive<R, W>(from: R, into: W) -> io::Result<usize>
       where R: io::Read + io::Write, W: io::Write
    {
        Xmodem::receive_with_progress(from, into, progress::noop)
    }

    /// XMODEMプロトコルを使用して `from` から `data` を受け取り、
    /// `into` に書き込む. `from` から読み込んだバイト数（128の倍数）を返す。
    ///
    /// 関数 `f` は、受信の進行状況を示すコールバックとして使用される。
    pub fn receive_with_progress<R, W>(from: R, mut into: W, f: ProgressFn) -> io::Result<usize>
       where R: io::Read + io::Write, W: io::Write
    {
        let mut receiver = Xmodem::new_with_progress(from, f);
        let mut packet = [0u8; 128];
        let mut received = 0;
        'next_packet: loop {
            for _ in 0..10 {
                match receiver.read_packet(&mut packet) {
                    Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                    Ok(0) => break 'next_packet,
                    Ok(n) => {
                        received += n;
                        into.write_all(&packet)?;
                        continue 'next_packet;
                    }
                }
            }

            return ioerr!(BrokenPipe, "bad receive");
        }

        Ok(received)
    }
}

fn get_checksum(buf: &[u8]) -> u8 {
    return buf.iter().fold(0, |a, b| a.wrapping_add(*b));
}

impl<T: io::Read + io::Write> Xmodem<T> {
    /// 内部リーダ/ライタを `inner` に設定した新しい `Xmodem`
    /// インスタンスを返す. 返されたインスタンスは受信
    ///  (ダウンロード) にも送信 (アップロード) にも使用できる。
    pub fn new(inner: T) -> Self {
        Xmodem { packet: 1, started: false, inner, progress: progress::noop}
    }

    /// 内部リーダ/ライタを `inner` に設定した新しい `Xmodem`
    /// インスタンスを返す. 返されたインスタンスは受信
    ///  (ダウンロード) にも送信 (アップロード) にも使用できる。
    /// 関数 `f` は、受信の進行状況を示すコールバックとして使用される。
    pub fn new_with_progress(inner: T, f: ProgressFn) -> Self {
        Xmodem { packet: 1, started: false, inner, progress: f }
    }

    /// 内部のI/Oストリームから1バイトを読み込む.
    /// `abort_on_can`が`true`の場合、読み込んだバイトが
    /// `CAN`の場合、`ConnectionAborted`のエラーを返す。
    ///
    /// # Errors
    ///
    /// 内部ストリームからの読み取りに失敗した場合、または、
    /// `abort_on_can`が`true`で読み取りバイト数が`CAN`の
    /// 場合はエラーを返す。
    fn read_byte(&mut self, abort_on_can: bool) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.inner.read_exact(&mut buf)?;

        let byte = buf[0];
        if abort_on_can && byte == CAN {
            return ioerr!(ConnectionAborted, "received CAN");
        }

        Ok(byte)
    }

    /// 内部I/Oストリームに1バイト書き込む.
    ///
    /// # Errors
    ///
    /// 内部ストリームへの書き込みに失敗した場合はエラーを返す。
    fn write_byte(&mut self, byte: u8) -> io::Result<()> {
        self.inner.write_all(&[byte])
    }

    /// 内部I/Oストリームから1バイトを読み込み、`byte`と比較する。
    /// バイトが一致した場合、そのバイトを`Ok`として返す。両者が
    /// 異なり、読み込んだバイトが`CAN`でない場合、`expected`を
    /// メッセージとする`InvalidData`エラーを返す。両者が異なり、
    /// 読み込んだバイトが`CAN`である場合は`ConnectionAborted`
    /// エラーを返す。どちらの場合もバイトが異なっていれば`CAN`
    /// バイトを内部ストリームに書き出す。
    ///
    /// # Errors
    ///
    /// 内部ストリームからの読み込みに失敗した場合、読み込んだ
    /// バイトが`byte`でなかった場合、読み込んだバイトが`CAN`で
    /// `byte`が`CAN`でなかった場合、バイトの不一致による`CAN`
    /// バイトの書き込みに失敗した場合はエラーを返す。
    fn expect_byte_or_cancel(&mut self, byte: u8, expected: &'static str) -> io::Result<u8> {
        let res = self.expect_byte(byte, expected);
        if res.is_err() {
            self.inner.write_all(&[CAN])?;
        }
        res
    }

    /// 内部I/Oストリームから1バイトを読み込み、`byte`と比較する。
    /// 両者が異なる場合、`expected`をメッセージとする`InvalidData`
    /// エラーを返す。それ以外は読み込んだバイトを返す。`byte`が`CAN`で
    /// なく読み込んだバイトが`CAN`の場合は`ConnectionAborted`
    /// エラーを返す。
    ///
    /// # Errors
    ///
    /// 内部ストリームからの読み込みに失敗した場合、読み込んだ
    /// バイトが`byte`でなかった場合、読み込んだバイトが`byte`と
    /// ことなり`CAN`であった場合、`ConnectionAborted`エラーを返す。
    /// それ以外は`InvalidData`エラーとする。
    fn expect_byte(&mut self, byte: u8, expected: &'static str) -> io::Result<u8> {
        let read = self.read_byte(false)?;
        if read == byte {
            Ok(read)
        } else if read == CAN {
            ioerr!(ConnectionAborted, "received CAN")
        } else {
            ioerr!(InvalidData, expected)
        }
    }

    /// XMODEMプロトコルを使用して内部ストリームからパケットを1つ
    /// 読み込む (ダウンロードする)。成功した場合は読み込んだ
    /// バイト数(常に128)を返す。
    ///
    /// 最初のパケットの受信を開始したら進捗コールバックを
    /// `Progress::Started`で呼び出す。後続のパケットの受信に
    /// 成功した場合は`Progress::Packet`で呼び出す。
    ///
    /// # Errors
    ///
    /// 内部ストリームの読み書きに失敗した場合はエラーを返す。
    /// また、XMODEMプロトコルがエラーを示している場合もエラーを
    /// 返す。特に、以下の場合は`InvalidData`エラーを返す。
    ///
    ///   * 送信者のパケットの最初のバイトが`EOT`でも`SOH`でもない。
    ///   * 送信者が最初の`EOT`に続いて2回目の`EOT`を送信しない。
    ///   * 受信したパケット番号が期待する値と一致しない。
    ///
    /// パケットのチェックサムが異なる場合は`Interrupted`エラーを返す。
    ///
    /// 予想外の`CAN`バイトを受信した場合は`ConnectionAborted`エラーを返す。
    ///
    /// `buf.len() < 128` の場合は`UnexpectedEof`エラーを返す。
    pub fn read_packet(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.len() < 128 {
            return ioerr!(UnexpectedEof, "buf.len() < 128");
        }

        if !self.started {
            self.write_byte(NAK)?;
            self.started = true;
        }

        match self.read_byte(true)? {
            SOH => {
                (self.progress)(Progress::Started);

                self.expect_byte_or_cancel(self.packet, "receive correct pno")?;
                self.expect_byte_or_cancel(255 - self.packet, "receive 255 - pno")?;

                for i in 0..128 {
                    buf[i] = self.read_byte(false)?;
                }

                let cs = self.read_byte(false)?;
                if cs != get_checksum(buf) {
                    self.write_byte(NAK)?;
                    return ioerr!(Interrupted, "not match checksum");
                }

                self.write_byte(ACK)?;
                (self.progress)(Progress::Packet(self.packet));
                self.packet += 1;
                Ok(128)
            }
            EOT => {
                self.write_byte(NAK)?;
                self.expect_byte_or_cancel(EOT, "expect EOT")?;
                self.write_byte(ACK)?;
                Ok(0)
            }
            _ => {
                self.write_byte(CAN)?;
                return ioerr!(InvalidData, "not SOH nor EOT");
            }
        }
    }

    /// XMODEM プロトコルを使用して内部ストリームにパケットを1つ
    /// 送信 (アップロード) する。`buf`が空の場合は送信終了を送信する。
    /// このインタフェースのユーザはデータ送信が完了した際に
    /// `write_packet(&[])`を呼ばなければならない。成功した場合は
    /// 書き込んだバイト数を返す。
    ///
    /// 受信者の`NAK`を待つ前に進捗コールバックを`Progress::Waiting`で
    /// 呼び出す。最初のパケットの送信を開始したら`Progress::Started`で、
    /// 後続のパケットの送信に成功した場合は`Progress::Packet`で呼び出す。
    ///
    /// # Errors
    ///
    /// 内部ストリームの読み書きに失敗した場合はエラーを返す。また、XMODEM
    /// プロトコルがエラーを示している場合もエラーを返す。特に、以下の場合は
    /// `InvalidData`エラーを返す。
    ///
    ///   * 受信者の最初のバイトが`NAK`でない。
    ///   * 受信者が最初の`EOT`に`NAK`で応答しない。
    ///   * 受信者が2回目の`EOT`に`ACK`で応答しない。
    ///   * 受信者が完全なパケットに対して`ACK`でも`NAK`でもない
    ///     バイトで応答した。
    ///
    /// `buf.len() < 128 && buf.len() != 0` の場合は
    /// `UnexpectedEof`エラーを返す。
    ///
    /// 予期しない`CAN`バイトを受信した場合は`
    /// `ConnectionAborted`エラーを返す。
    ///
    /// パケットのチェックサムが間違っている場合は
    /// `Interrupted`エラーを返す。
    pub fn write_packet(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() < 128 && buf.len() != 0 {
            return ioerr!(UnexpectedEof, "wrong buffer length");
        }

        (self.progress)(Progress::Waiting);

        if !self.started {
            self.expect_byte(NAK, "expect nak")?;
            self.started = true;
        }

        if buf.len() == 0 {
            self.write_byte(EOT)?;
            self.expect_byte(NAK, "expect nak")?;
            self.write_byte(EOT)?;
            self.expect_byte(ACK, "expect ack")?;
            self.started = false;
            return Ok(0);
        }

        (self.progress)(Progress::Started);

        self.write_byte(SOH)?;
        self.write_byte(self.packet)?;
        self.write_byte(255 - self.packet)?;

        for i in 0..128 {
            self.write_byte(buf[i])?;
        }
        self.write_byte(get_checksum(buf))?;

        match self.read_byte(true)? {
            ACK => {}
            NAK => return ioerr!(Interrupted, "replied NAK to checksum"),
            _ => return ioerr!(InvalidData, "invaled reply")
        }

        (self.progress)(Progress::Packet(self.packet));
        self.packet += 1;
        Ok(128)
    }


    /// この出力ストリームをフラッシュし、中間バッファリングされた
    /// コンテンツがすべて宛先に到達するようにする。
    ///
    /// # Errors
    ///
    /// I/Oエラーですべてのバイトを送信できなかった場合、または
    /// EOFが届いた場合はエラーとみなす。
    pub fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
