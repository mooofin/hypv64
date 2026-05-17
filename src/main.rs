#![no_std]
#![no_main]

#[macro_use]
mod print;

use core::arch::asm;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.boot")]
pub extern "C" fn boot() -> ! {
    unsafe {
        asm!(
            "la sp, __stack_top",
            "j {main}",
            main = sym main,
            options(noreturn)
        );
    }
}

/*
jump to the stack pointer and make it point to the top of the stack
then jump to the main function  and nvr return
*/

unsafe extern "C" {
    static mut __bss: u8;
    static mut __bss_end: u8;
}

fn main() -> ! {
    unsafe {
        let bss_start = &raw mut __bss;
        let bss_size = (&raw mut __bss_end as usize) - (bss_start as usize); /*zero everything for rodata */
        core::ptr::write_bytes(bss_start, 0, bss_size);
    }
    print::sbi_putchar(b'm');
    print::sbi_putchar(b'u');
    print::sbi_putchar(b'f');
    print::sbi_putchar(b'f');
    print::sbi_putchar(b'i');
    print::sbi_putchar(b'n');
    println!("\nBooting muffin hypervisor...");
    loop {}
}
use core::panic::PanicInfo;

#[panic_handler]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}

/*trap handler
 * when the cpu encounters an event that the OS needs to handle
 * it jumps to this function and handles it there
 */

mod trap;
