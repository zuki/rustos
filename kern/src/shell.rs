use shim::io;
use shim::path::{Path, PathBuf};
use shim::ffi::{OsStr, OsString};
use alloc::string::String;

use stack_vec::StackVec;
use core::str;

use pi::atags::Atags;

use fat32::traits::FileSystem;
use fat32::traits::{Dir, Entry, File};
use fat32::traits::metadata::{Metadata, Timestamp};
use crate::fs::PiVFatHandle;

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

fn canonicalize(cwd: &PathBuf) -> PathBuf {
    let mut path = PathBuf::new();
    for c in cwd.iter() {
        match c.to_str() {
            Some(".") => (),
            Some("..") => {
                let _ = path.pop();
            }
            Some(c) => path.push(c),
            None => (),
        }
    }
    path
}

fn print_ls_entry(entry: fat32::vfat::Entry<PiVFatHandle>, show_all: bool)  {
    if !show_all && (entry.metadata().attributes.hidden() || entry.name() == "." || entry.name() == "..") {
        return;
    }

    let mut line = String::new();
    if entry.is_dir() {
        line.push('d');
    } else {
        line.push('-');
    }

    if entry.metadata().hidden() {
        line.push('h');
    } else {
        line.push('-');
    }
    if entry.metadata().attributes.read_only() {
        line.push('r');
    } else {
        line.push('-');
    }

    let size = if entry.is_file() { entry.as_file().unwrap().size() } else { 0_u64 };

    kprintln!("{} {:>10} {} {}", line, size, entry.metadata().modified(), entry.name());
}

fn do_ls(cwd: &PathBuf, show_all: bool)  {
    let path = canonicalize(cwd);
    if let Ok(entry) = FILESYSTEM.open(path) {
        if entry.is_dir() {
            for e in entry.as_dir().unwrap().entries().unwrap() {
               print_ls_entry(e, show_all);
            }
        } else {
            print_ls_entry(entry, show_all);
        }
    } else {
        kprintln!("invalid path: {}", canonicalize(cwd).display());
    }
}

fn do_cat(path: &PathBuf)  {
    use io::Read;
    use io::Write;

    if let Ok(entry) = FILESYSTEM.open(canonicalize(path)) {
        if entry.is_file() {
            let mut buf = [0_u8; 512];
            let mut file = entry.into_file().unwrap();
            loop {
                match &file.read(&mut buf) {
                    Ok(bytes) => {
                        if *bytes == 0_usize {
                            break;
                        } else {
                            let mut console = CONSOLE.lock();
                            console.write(&buf[0..*bytes]);
                        }
                    }
                    Err(_) => {
                        kprintln!("read error occured");
                        break;
                    }
                }
            }
            kprintln!("");
        } else {
            kprintln!("{} is not file", canonicalize(path).display())
        }
    } else {
        kprintln!("{} is not exist", canonicalize(path).display());
    }
}

/// 各行のプリフィックスとして`prefix`を使ってシェルを開始する。
/// `exit`コマンドが呼び出されたらこの関数はリターンする。
pub fn shell(prefix: &str) -> () {
    let mut lbuf = [0u8; 512];
    let mut line = StackVec::new(&mut lbuf);
    let mut cwd = PathBuf::from("/");

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
                            &"cwd" => {
                                kprintln!("\n{}", cwd.display());
                            }
                            &"cd" => {
                                kprint!("\n");
                                if command.args.len() != 2 {
                                    kprintln!("cd requires <directory>");
                                } else {
                                    cwd.push(command.args[1]);
                                    cwd = canonicalize(&cwd);
                                }
                            }
                            &"cat" => {
                                kprint!("\n");
                                if command.args.len() < 2 {
                                    kprintln!("cat requires at least one <path>");
                                } else {
                                    for i in 1..command.args.len() {
                                        let path = &cwd.join(PathBuf::from(command.args[i]));
                                        do_cat(&path);
                                    }
                                }
                            }
                            &"ls" => {
                                kprint!("\n");
                                match command.args.len() {
                                    3 => {
                                        if command.args[1] != "-a" {
                                            kprintln!("bad arguments");
                                        } else {
                                            let dir = &cwd.join(PathBuf::from(command.args[2]));
                                            do_ls(&dir, true);
                                        }
                                    }
                                    2 => {
                                        if command.args[1] ==  "-a" {
                                            do_ls(&cwd, true);
                                        } else {
                                            let dir = &cwd.join(PathBuf::from(command.args[1]));
                                            do_ls(&dir, false);
                                        }
                                    }
                                    1 => {
                                        do_ls(&cwd, false);
                                    }
                                    _ => kprintln!("too many args"),
                                }
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
