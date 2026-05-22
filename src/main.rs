#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod print;

use core::arch::asm;

use crate::{
    allocator::alloc_pages,
    guest_page_table::{GuestPageTable, PTE_R, PTE_W, PTE_X},
};

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
    static mut __heap: u8;
    static mut __heap_end: u8;
}

fn main() -> ! {
    unsafe {
        let bss_start = &raw mut __bss;
        let bss_size = (&raw mut __bss_end as usize) - (bss_start as usize);
        core::ptr::write_bytes(bss_start, 0, bss_size);
    }

    allocator::GLOBAL_ALLOCATOR.init(&raw mut __heap, &raw mut __heap_end);

    println!("\nBooting muffin hypervisor...");

    /*
     * load our tiny guest kernel and stick it in page-aligned memory .
     * include_bytes! bakes guest.bin into the hypervisor binary at compile time .
     */
    let kernel_image = include_bytes!("../guest.bin");
    let guest_entry = 0x100000;
    let kernel_len = (kernel_image.len() + 4095) & !4095; // round up to page

    let kernel_memory = alloc_pages(kernel_len);
    unsafe {
        let dst = kernel_memory as *mut u8;
        let src = kernel_image.as_ptr();
        core::ptr::copy_nonoverlapping(src, dst, kernel_image.len());
    }

    /*
     * build a guest page table that maps guest virtual → guest physical ,
     * then set hgatp so the MMU knows where it is .
     */
    let mut table = GuestPageTable::new();
    table.map(guest_entry, kernel_memory as u64, PTE_R | PTE_W | PTE_X);
    let hgatp = table.hgatp();

    /*
     * hstatus tells the CPU what state to resume in .
     * SPV=1 means we enter VS-mode (virtualized supervisor) .
     * VSXL=2 means 64-bit for the guest .
     * sret pops sepc into pc and enters guest mode :3
     */
    let mut hstatus: u64 = 0;
    hstatus |= 2u64 << 32; // VSXL
    hstatus |= 1u64 << 7;  // SPV

    unsafe {
        asm!(
            "csrw hstatus, {hstatus}",
            "csrw hgatp, {hgatp}",
            "csrw sepc, {sepc}",
            "sret",
            hstatus = in(reg) hstatus,
            hgatp = in(reg) hgatp,
            sepc = in(reg) guest_entry,
        );
    }

    unreachable!();
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

mod allocator;

mod guest_page_table;
