// linux_loader.rs - Loading the Linux kernel and building the device tree
//
// Linux needs two things from the hypervisor:
//   1. Memory mapped to its address space (code + data)
//   2. A device tree (DTB) describing the hardware it's running on
//
// The device tree is like a JSON config for the kernel -- CPU type, memory
// ranges, interrupt controller, etc. Without it, Linux doesn't know what
// hardware is available.
//
// Rust's vm-fdt crate builds the FDT (Flattened Device Tree) binary.

use alloc::format;
use alloc::vec::Vec;

use crate::allocator::alloc_pages;
use crate::guest_page_table::{GuestPageTable, PTE_R, PTE_W, PTE_X};
use core::mem::size_of;

#[repr(C)]
struct RiscvImageHeader {
    code0: u32,
    code1: u32,
    text_offset: u64,
    image_size: u64,
    flags: u64,
    version: u32,
    reserved1: u32,
    reserved2: u64,
    magic: u64,
    magic2: u32,
    reserved3: u32,
}

pub const GUEST_BASE_ADDR: u64 = 0x8000_0000;
pub const GUEST_DTB_ADDR: u64 = 0x7000_0000;
pub const MEMORY_SIZE: usize = 8 * 1024 * 1024;

pub const PLIC_ADDR: u64 = 0x0c00_0000;
pub const PLIC_END: u64 = PLIC_ADDR + 0x400000;

// copy_and_map: Allocate + copy + map in one shot
//
// Three steps, all in one function:
//   1. alloc_pages(len) -- grab page-aligned memory from the hypervisor heap
//   2. copy_nonoverlapping -- copy the data in (like memcpy)
//   3. Walk 4 KiB at a time, table.map() each page so the guest MMU can find it
//
// flags is a permission bitmask (PTE_R | PTE_W | PTE_X for read/write/execute).
// The guest sees this as physical memory starting at guest_addr.
fn copy_and_map(
    table: &mut GuestPageTable,
    data: &[u8],
    guest_addr: u64,
    len: usize,
    flags: u64,
) {
    assert!(data.len() <= len, "data is beyond the region");
    let raw_ptr = alloc_pages(len);
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), raw_ptr, data.len());
    }

    let host_addr = raw_ptr as u64;
    for off in (0..len).step_by(4096) {
        table.map(guest_addr + off as u64, host_addr + off as u64, flags);
    }
}

fn build_device_tree() -> Result<Vec<u8>, vm_fdt::Error> {
    let mut fdt = vm_fdt::FdtWriter::new()?;
    let root_node = fdt.begin_node("")?;
    fdt.property_string("compatible", "riscv-virtio")?;
    fdt.property_u32("#address-cells", 0x2)?;
    fdt.property_u32("#size-cells", 0x2)?;

    let chosen_node = fdt.begin_node("chosen")?;
    fdt.property_string("bootargs", "console=hvc earlycon=sbi panic=-1")?;
    fdt.end_node(chosen_node)?;

    let mem_name = format!("memory@{:x}", GUEST_BASE_ADDR);
    let memory_node = fdt.begin_node(&mem_name)?;
    fdt.property_string("device_type", "memory")?;
    fdt.property_array_u64("reg", &[GUEST_BASE_ADDR, MEMORY_SIZE as u64])?;
    fdt.end_node(memory_node)?;

    let cpus_node = fdt.begin_node("cpus")?;
    fdt.property_u32("#address-cells", 0x1)?;
    fdt.property_u32("#size-cells", 0x0)?;
    fdt.property_u32("timebase-frequency", 10000000)?;

    let cpu_node = fdt.begin_node("cpu@0")?;
    fdt.property_string("device_type", "cpu")?;
    fdt.property_string("compatible", "riscv")?;
    fdt.property_u32("reg", 0)?;
    fdt.property_string("status", "okay")?;
    fdt.property_string("mmu-type", "riscv,sv48")?;
    fdt.property_string("riscv,isa", "rv64imafdc")?;

    let intc_node = fdt.begin_node("interrupt-controller")?;
    fdt.property_u32("#interrupt-cells", 1)?;
    fdt.property_null("interrupt-controller")?;
    fdt.property_string("compatible", "riscv,cpu-intc")?;
    fdt.property_u32("phandle", 1)?;
    fdt.end_node(intc_node)?;

    fdt.end_node(cpu_node)?;
    fdt.end_node(cpus_node)?;

    let plic_node = fdt.begin_node("plic@c000000")?;
    fdt.property_string("compatible", "riscv,plic0")?;
    fdt.property_u32("#interrupt-cells", 1)?;
    fdt.property_null("interrupt-controller")?;
    fdt.property_array_u64("reg", &[PLIC_ADDR, 0x4000000])?;
    fdt.property_u32("riscv,ndev", 3)?;
    fdt.property_array_u32("interrupts-extended", &[1, 11, 1, 9])?;
    fdt.property_u32("phandle", 2)?;
    fdt.end_node(plic_node)?;

    fdt.end_node(root_node)?;
    fdt.finish()
}

// load_linux_kernel: Put the Linux kernel in guest memory
//
// The Linux image (from linux/Image) has a binary header with:
//   - magic: validates the loader
//   - text_offset: where code starts
//   - image_size: total kernel size
//   - flags: build flags
//
// We check the magic to ensure we're not running garbage. If valid, we allocate
// page-aligned memory, copy the image in, and map each 4 KiB page. Then we
// add the device tree (DTB) and print the size so we know we loaded it.
//
// This is like how C++ constructors validate object state before initializing.
pub fn load_linux_kernel(table: &mut GuestPageTable, image: &[u8]) {
    assert!(image.len() >= size_of::<RiscvImageHeader>());
    let header = unsafe { &*(image.as_ptr() as *const RiscvImageHeader) };
    assert_eq!(u32::from_le(header.magic2), 0x05435352, "invalid magic");

    let kernel_size = u64::from_le(header.image_size);
    assert!(image.len() <= MEMORY_SIZE);
    copy_and_map(
        table,
        image,
        GUEST_BASE_ADDR,
        MEMORY_SIZE,
        PTE_R | PTE_W | PTE_X,
    );

    let dtb = build_device_tree().unwrap();
    assert!(dtb.len() <= 0x10000, "DTB is too large");
    copy_and_map(table, &dtb, GUEST_DTB_ADDR, dtb.len(), PTE_R);

    println!("loaded kernel: size={}KB", kernel_size / 1024);
}
