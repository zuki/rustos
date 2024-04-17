#![feature(panic_info_message)]
use core::panic::PanicInfo;
use crate::console::kprintln;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kprintln!("            (");
    kprintln!("       (      )     )");
    kprintln!("         )   (    (");
    kprintln!("        (          `");
    kprintln!("    .-\"\"^\"\"\"^\"\"^\"\"\"^\"\"-.");
    kprintln!("  (//\\\\//\\\\//\\\\//\\\\//\\\\//)");
    kprintln!("   ~\\^^^^^^^^^^^^^^^^^^/~");
    kprintln!("     `================`");
    kprintln!("");
    kprintln!("    The pi is overdone.");
    kprintln!("");
    kprintln!("---------- PANIC ----------");
    kprintln!("");

    if let Some(location) = info.location() {
        kprintln!("FILE: {}", location.file());
        kprintln!("LINE: {}", location.line());
        kprintln!("COL: {}", location.column());
        kprintln!("");
    }

    if let Some(message) = info.message() {
        kprintln!("{}", message);
    }

    loop {}
}
