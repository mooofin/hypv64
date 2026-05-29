/*simple bump allocator ,
 * we cannot reuse free memory for now
 */

use core::alloc::{GlobalAlloc, Layout};
/*trait for defining how memory alloc works  */

struct Mutable {
    next: usize,
    end: usize,
    /*usize is always pointer sized  */
}

pub struct BumpAllocator {
    mutable: spin::Mutex<Option<Mutable>>,
    /*only one thread touches next at a time */
}

impl BumpAllocator {
    const fn new() -> Self {
        Self {
            mutable: spin::Mutex::new(None),
        }
    }
    /*
    * const for static global variable , this has to run before main ,
    we need to alloc memory , but this becomes a chicken egg probelem.
    so an object allocator exists as data with nOne params , with zero cost runtime
    */
    pub fn init(&self, start: *mut u8, end: *mut u8) {
        self.mutable.lock().replace(Mutable {
            next: start as usize,
            end: end as usize,
        });
    }

    /*
     * init takes 2 pointers for the start and end for the pre alloced arena heap .
     * lock the internal spinlock then replace for state change :)
     * the qsn of why is the init is ,without it the compile time state "none"
     * would be created and , any attempt will panic
     */
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut mutable_lock = self.mutable.lock();
        /*layout has 2 fields size and align  */
        let mutable = mutable_lock
            .as_mut()
            .expect("muffin allocator not initialized");
        /*acuirw the spinlock , if another thread is in alloc we wait
         * return a guard that drops the lock when it drops
         */
        let addr = mutable.next.next_multiple_of(layout.align());
        assert!(
            addr.saturating_add(layout.size()) <= mutable.end,
            "out of memory"
        );

        mutable.next = addr + layout.size();
        addr as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
#[global_allocator]
pub static GLOBAL_ALLOCATOR: BumpAllocator = BumpAllocator::new();
/*wires through whenever we need heap allocs */

/*
 * alloc_pages gives you zeroed page-aligned memory .
 * layout bundles size + 4096-alignment for the allocator .
 * alloc_zeroed calls alloc + memset(0) so guests dont see this
 */
pub fn alloc_pages(len: usize) -> *mut u8 {
    debug_assert!(len % 4096 == 0, "len must be a multiple of 4096");
    let layout = Layout::from_size_align(len, 4096).unwrap();
    unsafe { GLOBAL_ALLOCATOR.alloc(layout) as *mut u8 }
}
