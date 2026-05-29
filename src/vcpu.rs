use alloc::vec::Vec;
use core::arch::asm;
use spin::Mutex;

use crate::allocator::alloc_pages;
use crate::guest_page_table::GuestPageTable;

static CONSOLE_BUFFER: Mutex<Vec<u8>> = Mutex::new(Vec::new());

const TRAP_STACK_SIZE: usize = 4096;
static mut TRAP_STACK: Option<*mut u8> = None;

/// Returns a pointer to a 4 KiB trap stack, allocating it on first call.
fn trap_stack() -> *mut u8 {
    unsafe {
        match TRAP_STACK {
            Some(ptr) => ptr,
            None => {
                let ptr = alloc_pages(TRAP_STACK_SIZE);
                TRAP_STACK = Some(ptr);
                ptr
            }
        }
    }
}

core::arch::global_asm!(
    ".align 2",
    ".globl trap_vector",
    "trap_vector:",
    "  csrrw sp, sscratch, sp",
    "  sd ra, 0x00(sp)",
    "  sd gp, 0x08(sp)",
    "  sd tp, 0x10(sp)",
    "  sd t0, 0x18(sp)",
    "  sd t1, 0x20(sp)",
    "  sd t2, 0x28(sp)",
    "  sd s0, 0x30(sp)",
    "  sd s1, 0x38(sp)",
    "  sd a0, 0x40(sp)",
    "  sd a1, 0x48(sp)",
    "  sd a2, 0x50(sp)",
    "  sd a3, 0x58(sp)",
    "  sd a4, 0x60(sp)",
    "  sd a5, 0x68(sp)",
    "  sd a6, 0x70(sp)",
    "  sd a7, 0x78(sp)",
    "  sd s2, 0x80(sp)",
    "  sd s3, 0x88(sp)",
    "  sd s4, 0x90(sp)",
    "  sd s5, 0x98(sp)",
    "  sd s6, 0xa0(sp)",
    "  sd s7, 0xa8(sp)",
    "  sd s8, 0xb0(sp)",
    "  sd s9, 0xb8(sp)",
    "  sd s10, 0xc0(sp)",
    "  sd s11, 0xc8(sp)",
    "  sd t3, 0xd0(sp)",
    "  sd t4, 0xd8(sp)",
    "  sd t5, 0xe0(sp)",
    "  sd t6, 0xe8(sp)",
    "  csrr a0, sscratch",
    "  sd a0, 0xf0(sp)",
    "  addi a0, sp, 0x100",
    "  csrw sscratch, a0",
    "  mv a0, sp",
    "  call handle_trap",
    "  ld ra, 0x00(sp)",
    "  ld gp, 0x08(sp)",
    "  ld tp, 0x10(sp)",
    "  ld t0, 0x18(sp)",
    "  ld t1, 0x20(sp)",
    "  ld t2, 0x28(sp)",
    "  ld s0, 0x30(sp)",
    "  ld s1, 0x38(sp)",
    "  ld a0, 0x40(sp)",
    "  ld a1, 0x48(sp)",
    "  ld a2, 0x50(sp)",
    "  ld a3, 0x58(sp)",
    "  ld a4, 0x60(sp)",
    "  ld a5, 0x68(sp)",
    "  ld a6, 0x70(sp)",
    "  ld a7, 0x78(sp)",
    "  ld s2, 0x80(sp)",
    "  ld s3, 0x88(sp)",
    "  ld s4, 0x90(sp)",
    "  ld s5, 0x98(sp)",
    "  ld s6, 0xa0(sp)",
    "  ld s7, 0xa8(sp)",
    "  ld s8, 0xb0(sp)",
    "  ld s9, 0xb8(sp)",
    "  ld s10, 0xc0(sp)",
    "  ld s11, 0xc8(sp)",
    "  ld t3, 0xd0(sp)",
    "  ld t4, 0xd8(sp)",
    "  ld t5, 0xe0(sp)",
    "  ld t6, 0xe8(sp)",
    "  csrrw sp, sscratch, sp",
    "  sret",
);

pub struct VCpu {
    pub regs: [u64; 32],
    pub sepc: u64,
    pub hedeleg: u64,
    pub hgatp: u64,
}

impl VCpu {
    pub fn new(table: &GuestPageTable, guest_entry: u64) -> Self {
        let mut hedeleg: u64 = 0;
        hedeleg |= 1 << 0;
        hedeleg |= 1 << 1;
        hedeleg |= 1 << 2;
        hedeleg |= 1 << 3;
        hedeleg |= 1 << 4;
        hedeleg |= 1 << 5;
        hedeleg |= 1 << 6;
        hedeleg |= 1 << 7;
        hedeleg |= 1 << 8;
        hedeleg |= 1 << 10;
        hedeleg |= 1 << 12;
        hedeleg |= 1 << 16;
        hedeleg |= 1 << 17;
        hedeleg |= 1 << 22;
        hedeleg |= 1 << 13;
        hedeleg |= 1 << 15;
        hedeleg |= 1 << 29;
        hedeleg |= 1 << 31;

        Self {
            regs: [0; 32],
            sepc: guest_entry,
            hedeleg,
            hgatp: table.hgatp(),
        }
    }

    pub fn a0(&self) -> u64 {
        self.regs[10]
    }
    pub fn set_a0(&mut self, val: u64) {
        self.regs[10] = val;
    }
    pub fn a1(&self) -> u64 {
        self.regs[11]
    }
    pub fn set_a1(&mut self, val: u64) {
        self.regs[11] = val;
    }
    pub fn a6(&self) -> u64 {
        self.regs[16]
    }
    pub fn a7(&self) -> u64 {
        self.regs[17]
    }

    // run: Handover from host to guest
    //
    // This function sets up the CPU for VS-mode (Virtualized Supervisor mode).
    // The host (hypervisor) tells the CPU:
    // 1. stvec = trap_vector (where to handle traps)
    // 2. hstatus = enable VS-mode (bit 7)
    // 3. hgatp = guest page table (where code/data are)
    // 4. hedeleg = which traps to delegate to us
    // 5. hcounteren = enable counters (time/cycle)
    // 6. GS bit = M-mode E-call flag
    // 7. sepc = guest kernel entry point (0x80000000)
    //
    // After "sret", we're no longer the hypervisor -- the guest runs.
    pub fn run(&mut self) -> ! {
        unsafe extern "C" {
            fn trap_vector();
        }
        let hstatus: u64;
        unsafe { asm!("csrr {0}, hstatus", out(reg) hstatus) }
        let hstatus = hstatus | (1u64 << 7);
        let trap_stack_top = trap_stack() as u64 + TRAP_STACK_SIZE as u64;
        unsafe {
            asm!(
                "csrw stvec, {trap_vector}",
                "csrw sscratch, {trap_stack}",
                "csrw hstatus, {hstatus}",
                "csrw hgatp, {hgatp}",
                "csrw hedeleg, {hedeleg}",
                "csrw hcounteren, {hcounteren}",
                "csrw sepc, {sepc}",
                "sret",
                trap_vector = in(reg) trap_vector,
                trap_stack = in(reg) trap_stack_top,
                hstatus = in(reg) hstatus,
                hgatp = in(reg) self.hgatp,
                hedeleg = in(reg) self.hedeleg,
                hcounteren = in(reg) 0b11,
                sepc = in(reg) self.sepc,
                options(noreturn)
            );
        }
    }
}

// handle_trap: The hypervisor's door back from the guest
//
// When the guest hits an exception (page fault, ecall, whatever), the CPU
// switches to HS mode and jumps right here. Three CSRs tell us what happened:
//   scause = why (page fault? SBI call? illegal instruction?)
//   sepc = where in guest code it happened
//   stval = optional extra info (faulting address, etc.)
//
// All guest registers were saved by trap_vector (the assembly stub), so regs
// is a pointer to that saved state. We can read and modify guest regs here.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn handle_trap(regs: &mut [u64; 32]) {
    let scause: u64;
    let sepc: u64;
    let stval: u64;
    unsafe {
        asm!(
            "csrr {0}, scause",
            "csrr {1}, sepc",
            "csrr {2}, stval",
            out(reg) scause,
            out(reg) sepc,
            out(reg) stval,
        )
    }

    match scause {
        // scause=9: E-call from HS-mode. Our own println() does ecall to OpenSBI
        // via medeleg. OpenSBI prints the char and returns. If for some reason
        // we intercept it, just advance sepc and move on.
        9 => {
            let next_sepc = sepc + 4;
            unsafe { asm!("csrw sepc, {0}", in(reg) next_sepc) }
        }
        // scause=10: E-call from VS-mode. The guest wants us to do something
        // (print, set timer, probe extensions). Dispatch to handle_sbi_call()
        // which matches on a7 (EID) and a6 (FID).
        10 => {
            handle_sbi_call(regs);
            let next_sepc = sepc + 4;
            unsafe { asm!("csrw sepc, {0}", in(reg) next_sepc) }
        }
        // scause=22: Guest tried a privileged instruction (e.g., csrw sie).
        // Skip for now and let the guest continue.
        22 => {
            let next_sepc = sepc + 4;
            unsafe { asm!("csrw sepc, {0}", in(reg) next_sepc) }
        }
        // scause=13/15: Guest accessed memory we didn't map.
        13 | 15 => {
            let scause_str = if scause == 13 {
                "load guest-page fault"
            } else {
                "store/AMO guest-page fault"
            };
            panic!(
                "trap handler: {} at {:#x} (stval={:#x})",
                scause_str, sepc, stval
            );
        }
        _ => {
            let scause_str = match scause {
                0 => "instruction address misaligned",
                1 => "instruction access fault",
                2 => "illegal instruction",
                3 => "breakpoint",
                4 => "load address misaligned",
                5 => "load access fault",
                6 => "store/AMO address misaligned",
                7 => "store/AMO access fault",
                8 => "environment call from U/VU-mode",
                9 => "environment call from HS-mode",
                11 => "environment call from M-mode",
                12 => "instruction page fault",
                20 => "instruction guest-page fault",
                21 => "load guest-page fault",
                22 => "virtual instruction",
                23 => "store/AMO guest-page fault",
                _ => "unknown",
            };
            panic!(
                "trap handler: {} at {:#x} (stval={:#x})",
                scause_str, sepc, stval
            );
        }
    }
}

// This function is called when the guest makes an SBI call (e.g., to print,
// set timer, probe extensions). It matches on e7 (extension ID) and a6
// (function ID) and returns the result.
//
// The pattern looks like this:
//   e7 = 0x01 (console extension)
//   a6 = 0x00 (putchar function)
//   a0 = character to print
//
// If we don't recognize it, we panic.
fn handle_sbi_call(regs: &mut [u64; 32]) {
    let eid = regs[17];
    let fid = regs[16];
    let result: Result<i64, i64> = match (eid, fid) {
        (0x10, 0x0) => Ok(0),
        (0x10, 0x3) => Err(-1),
        (0x10, 0x4 | 0x5 | 0x6) => Ok(0),
        (0x00, 0x0) => {
            println!("[sbi] WARN: set_timer is not implemented, ignoring");
            Ok(0)
        }
        (0x01, 0x0) => {
            let ch = regs[10] as u8;
            let mut buffer = CONSOLE_BUFFER.lock();
            if ch == b'\n' {
                let output = core::str::from_utf8(&buffer).unwrap_or("(not utf-8)");
                println!("[guest] {}", output);
                buffer.clear();
            } else {
                buffer.push(ch);
            }
            Ok(0)
        }
        _ => {
            panic!("unknown SBI call: eid={:#x}, fid={:#x}", eid, fid);
        }
    };

    match result {
        Ok(value) => {
            regs[10] = 0;
            regs[11] = value as u64;
        }
        Err(err) => {
            regs[10] = err as u64;
        }
    }
}
