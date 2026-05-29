#![no_std]
#![no_main]

extern crate alloc;

#[macro_use]
mod print;

use core::arch::asm;

use crate::guest_page_table::GuestPageTable;

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

    println!("\nBooting hypervisor...");

    let mut table = GuestPageTable::new();
    let kernel_image = include_bytes!("../linux/Image");
    linux_loader::load_linux_kernel(&mut table, kernel_image);

    let mut vcpu = vcpu::VCpu::new(&table, linux_loader::GUEST_BASE_ADDR);
    vcpu.set_a0(0);
    vcpu.set_a1(linux_loader::GUEST_DTB_ADDR);
    vcpu.run();
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

mod allocator;

mod guest_page_table;

mod linux_loader;

mod vcpu;
