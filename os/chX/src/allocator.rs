//! Buddy allocator — kernel heap.
//!
//! ## What you need to do
//!
//! Fill in the three `todo!()` blocks in the `BuddyAlloc` impl below:
//! `init`, `alloc`, and `dealloc`.
//!
//! ## Provided helpers (from `tg_buddy_alloc`)
//!
//! - `FreeNode::push(head, ptr)` / `FreeNode::pop(head)` / `FreeNode::remove(head, ptr)`
//! - `BuddyAllocator::block_size(order)` — `2^(order + MIN_ORDER)`
//! - `BuddyAllocator::buddy_addr(ptr, order)` — XOR trick
//! - `BuddyAllocator::size_to_order(size)` — byte size → order

use tg_buddy_alloc::{BuddyAlloc, BuddyAllocator, FreeNode, LockedBuddy, MAX_ORDER, MIN_ORDER};

/// Our allocator type — a thin wrapper around `BuddyAllocator`.
pub struct Allocator(pub BuddyAllocator);

impl Allocator {
    const fn new() -> Self {
        Self(BuddyAllocator::new())
    }
}

#[global_allocator]
static HEAP: LockedBuddy<Allocator> = LockedBuddy::new(Allocator::new());

/// Called once at boot to hand the heap region to the allocator.
///
/// # Safety
///
/// `base` and `size` must describe a valid, unused memory region.
pub unsafe fn init(base: usize, size: usize) {
    HEAP.get_mut().init(base, size);
}

// ── TODO: implement BuddyAlloc ─────────────────────────────────────────

impl BuddyAlloc for Allocator {
    fn init(&mut self, base: usize, size: usize) {
        // TODO: store base/total_size, then break the region into
        //       power-of-two blocks and push each onto the right free list.
        todo!()
    }

    fn alloc(&mut self, order: usize) -> *mut u8 {
        // TODO: pop from free_lists[order]; if empty, find a larger block
        //       and split it down.
        todo!()
    }

    fn dealloc(&mut self, ptr: *mut u8, order: usize) {
        // TODO: push onto free_lists[order], then check buddy — if free,
        //       remove it and merge; repeat upward.
        todo!()
    }
}
