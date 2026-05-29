use core::mem::size_of;

/*
 * page table entry (PTE) bit defs for riscv64 sv39
 * V=valid R=read W=write X=exec U=user
 * if none of R/W/X are set , its a pointer to next-level table not a leaf :3
 */
pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;
pub const PTE_U: u64 = 1 << 4;

const PPN_SHIFT: usize = 12;
const PTE_PPN_SHIFT: usize = 10;

/*
 * Entry wraps a u64 PTE .
 * repr(transparent) means Entry IS a u64 in memory
 * so we can cast between them safely for MMU to read
 */
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct Entry(u64);

impl Entry {
    /*
     * new packs a physical address + flags into a PTE
     * addr >> 12 = page number , then << 10 = position in PTE bits
     */
    pub fn new(paddr: u64, flags: u64) -> Self {
        let ppn = (paddr as u64) >> PPN_SHIFT;
        Self(ppn << PTE_PPN_SHIFT | flags)
    }

    pub fn is_valid(&self) -> bool {
        self.0 & PTE_V != 0
    }

    /*
     * paddr extracts the physical address back from a PTE
     * reverse of new : shift out flags , shift back to byte addr
     */
    pub fn paddr(&self) -> u64 {
        (self.0 >> PTE_PPN_SHIFT) << PPN_SHIFT
    }
}

/*
 * Table = one level of the page table = 512 entries = exactly 4KiB
 * alloc() hands us a zeroed page to fill in
 */
#[repr(transparent)]
struct Table([Entry; 512]);

impl Table {
    pub fn alloc() -> *mut Table {
        crate::allocator::alloc_pages(size_of::<Table>()) as *mut Table
    }

    /*
     * entry_by_addr extracts the 9-bit index for a given level
     * level 0 = leaf (4KiB pages)
     * level 1 = 2nd level (2MiB pages)
     * level 2 = 3rd level (1GiB pages)
     * each level shifts by 12 + 9*level bits
     */
    pub fn entry_by_addr(&mut self, guest_paddr: u64, level: usize) -> &mut Entry {
        let index = (guest_paddr >> (12 + 9 * level)) & 0x1ff;
        &mut self.0[index as usize]
    }
}

/*
 * GuestPageTable holds the root (top-level) table pointer
 * hgatp() returns the value to write into the hgatp CSR
 * map() walks levels 3→0 , creating intermediate tables asneeded
 */
pub struct GuestPageTable {
    table: *mut Table,
}

impl GuestPageTable {
    pub fn new() -> Self {
        Self {
            table: Table::alloc(),
        }
    }

    /*
     * hgatp format: mode(4 bits) | zeroes | PPN(44 bits)
     * mode 9 = Sv39x4 (Sv39 with 4 levels for virtualization)
     */
    pub fn hgatp(&self) -> u64 {
        (9u64 << 60) | (self.table as u64 >> PPN_SHIFT)
    }

    /*
     * map creates a mapping from guest_paddr → host_paddr
     * walks from top level (3) down to leaf (0)
     * if a mid-level table is missing , allocates one :3
     */
    pub fn map(&mut self, guest_paddr: u64, host_paddr: u64, flags: u64) {
        let mut table = unsafe { &mut *self.table };
        for level in (1..=3).rev() {
            let entry = table.entry_by_addr(guest_paddr, level);
            if !entry.is_valid() {
                let new_table_ptr = Table::alloc();
                *entry = Entry::new(new_table_ptr as u64, PTE_V);
            }
            table = unsafe { &mut *(entry.paddr() as *mut Table) };
        }

        let entry = table.entry_by_addr(guest_paddr, 0);
        assert!(!entry.is_valid(), "already mapped");
        *entry = Entry::new(host_paddr, flags | PTE_V | PTE_U);
    }
}
