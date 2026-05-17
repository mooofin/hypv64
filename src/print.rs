use core::arch::asm;

pub fn sbi_putchar(ch: u8) {
    unsafe {
        asm!(
            "ecall",
            in("a6") 0, // SBI function ID
            in("a7") 1, // SBI extension ID (Console Putchar)
            inout("a0") ch as usize => _, // Argument #0
            out("a1") _ // Argument #1 (not used)
        );
    }
}
/*ecall is an instruction to pauses user-level code, transfers control to a handler (here a supervisor ) */

pub struct Printer;

impl core::fmt::Write for Printer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for ch in s.bytes() {
            sbi_putchar(ch);
        }
        Ok(())
    }
}
/*
 * we need a writter trait for the printer
 * and to create our own println! macro for quality of life :3
 *
 */
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        let _ = writeln!($crate::print::Printer, $($arg)*);
    }};
}

/*
* this expands to {
    use core::fmt::Write;

    let _ =
        writeln!(
            Printer,
            "value={}",
            x
        );
}
*
*/
