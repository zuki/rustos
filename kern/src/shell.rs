use shim::io;
use shim::path::{Path, PathBuf};

use stack_vec::StackVec;
use core::str;

use pi::atags::Atags;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry};

use crate::console::{kprint, kprintln, CONSOLE};
use crate::ALLOCATOR;
use crate::FILESYSTEM;

/// `Command`のパースに失敗した際のエラー型.
#[derive(Debug)]
enum Error {
    Empty,
    TooManyArgs,
}

/// 1つのシェルコマンドを表す構造体.
struct Command<'a> {
    args: StackVec<'a, &'a str>,
}

impl<'a> Command<'a> {
    /// 文字列 `s` のコマンドをパースして引数を `buf` に格納する.
    ///
    /// # Errors
    ///
    /// If `s`に引数が含まれていない場合は `Error::Empty` を、
    /// `buf`が保存可能な数以上の引数が存在した場合は
    /// `Error::TooManyArgs` を返す。
    fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
        let mut args = StackVec::new(buf);
        for arg in s.split(' ').filter(|a| !a.is_empty()) {
            args.push(arg).map_err(|_| Error::TooManyArgs)?;
        }

        if args.is_empty() {
            return Err(Error::Empty);
        }

        Ok(Command { args })
    }

    /// このコマンドのパスを返す. これは第1引数に相当する。
    fn path(&self) -> &str {
        self.args.as_slice()[0]
    }
}

/// 各行のプリフィックスとして`prefix`を使ってシェルを開始する。
/// `exit`コマンドが呼び出されたらこの関数はリターンする。
pub fn shell(prefix: &str) -> () {
    let mut lbuf = [0u8; 512];
    let mut line = StackVec::new(&mut lbuf);

    kprint!("{}", prefix);
    loop {
        let mut console = CONSOLE.lock();
        let byte = console.read_byte();
        match byte {
            b'\r' | b'\n' => {
                let mut buf = ["0"; 64];
                match Command::parse(core::str::from_utf8(line.as_slice()).unwrap(), &mut buf) {
                    Ok(command) => {
                        let path = &command.path();
                        match path {
                            &"echo" => {
                                kprint!("\n");
                                for i in 1..command.args.len() {
                                    kprint!("{} ", &command.args[i]);
                                }
                                kprint!("\n");
                            }
                            &"exit" => {
                                kprintln!("\nexit");
                                return;
                            }
                            _ => kprintln!("\nunknown command: {}", path),
                        }
                    }
                    Err(e) => match e {
                        Error::Empty => kprint!("\n"),
                        Error::TooManyArgs => kprintln!("\nerror: too many arguments"),
                    },
                }
                line.truncate(0);
                kprint!("{}", prefix);
            }
            b'\x08' | b'\x7f' => {
                if line.len() != 0 {
                    line.pop();
                    &console.write_byte(b'\x08');
                    &console.write_byte(b'\x20');
                    &console.write_byte(b'\x08');
                }
            }
            b'\x00'..=b'\x19' => {
                &console.write_byte(b'\x07');
            }
            _ => {
                line.push(byte).unwrap();
                &console.write_byte(byte);
                if line.is_full() {
                    &console.write_byte(b'\x07');
                }
            }
        }
    }
}
