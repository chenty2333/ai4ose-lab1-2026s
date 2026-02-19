//! Buddy allocator framework for OS kernel memory management.
//!
//! Provides the scaffolding for a binary buddy allocator:
//! an intrusive free list ([`FreeNode`]), allocator state ([`BuddyAllocator`]),
//! a trait ([`BuddyAlloc`]) defining the core logic, and a generic
//! [`LockedBuddy<T>`] wrapper that bridges any `T: BuddyAlloc` to
//! [`GlobalAlloc`].
//!
//! Users define a type (typically wrapping [`BuddyAllocator`]),
//! implement [`BuddyAlloc`] for it, and declare a
//! `#[global_allocator] static HEAP: LockedBuddy<MyAllocator> = …;`

#![no_std]
#![deny(missing_docs)]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;

/// Bit width of the minimum allocation unit. Minimum block = 2^MIN_ORDER = 8 bytes.
pub const MIN_ORDER: usize = 3;

/// Maximum order relative to MIN_ORDER. Max block = 2^(3+27) = 1 GiB.
pub const MAX_ORDER: usize = 27;

// ── Intrusive free-list node ────────────────────────────────────────────

/// Intrusive free-list node embedded at the head of each free block.
///
/// When a block is allocated the node is naturally overwritten by user data;
/// on deallocation the node is written back.
#[repr(C)]
pub struct FreeNode {
    /// Next free block in the same order, null if tail.
    pub next: *mut FreeNode,
}

impl FreeNode {
    /// Push a free block onto the front of the list.
    ///
    /// # Safety
    ///
    /// `ptr` must point to at least `size_of::<FreeNode>()` bytes of writable memory.
    #[inline]
    pub unsafe fn push(head: &mut *mut FreeNode, ptr: *mut u8) {
        let node = ptr as *mut FreeNode;
        (*node).next = *head;
        *head = node;
    }

    /// Pop a free block from the front of the list. Returns `None` if empty.
    #[inline]
    pub fn pop(head: &mut *mut FreeNode) -> Option<*mut u8> {
        let node = *head;
        if node.is_null() {
            None
        } else {
            *head = unsafe { (*node).next };
            Some(node as *mut u8)
        }
    }

    /// Remove a specific block from the list. Returns `true` if found.
    pub fn remove(head: &mut *mut FreeNode, target: *mut u8) -> bool {
        let target_node = target as *mut FreeNode;
        if *head == target_node {
            *head = unsafe { (*target_node).next };
            return true;
        }
        let mut cur = *head;
        while !cur.is_null() {
            let next = unsafe { (*cur).next };
            if next == target_node {
                unsafe { (*cur).next = (*target_node).next };
                return true;
            }
            cur = next;
        }
        false
    }
}

// ── Allocator state ─────────────────────────────────────────────────────

/// Buddy allocator state.
///
/// Maintains `MAX_ORDER + 1` free lists where `free_lists[i]` tracks
/// blocks of size `2^(i + MIN_ORDER)`.
pub struct BuddyAllocator {
    /// Start address of the managed region.
    pub base: usize,
    /// Total size of the managed region in bytes.
    pub total_size: usize,
    /// Per-order free list heads.
    pub free_lists: [*mut FreeNode; MAX_ORDER + 1],
}

// SAFETY: single-core kernel, no concurrent access.
unsafe impl Send for BuddyAllocator {}

impl BuddyAllocator {
    /// Create an uninitialised allocator.
    pub const fn new() -> Self {
        Self {
            base: 0,
            total_size: 0,
            free_lists: [core::ptr::null_mut(); MAX_ORDER + 1],
        }
    }

    /// Block size in bytes for a given order: `2^(order + MIN_ORDER)`.
    #[inline]
    pub const fn block_size(order: usize) -> usize {
        1 << (order + MIN_ORDER)
    }

    /// Buddy address of `ptr` at `order`: `ptr ^ block_size(order)`.
    #[inline]
    pub const fn buddy_addr(ptr: usize, order: usize) -> usize {
        ptr ^ Self::block_size(order)
    }

    /// Smallest order whose block size >= `size`. Returns `None` if too large.
    #[inline]
    pub fn size_to_order(size: usize) -> Option<usize> {
        if size == 0 {
            return Some(0);
        }
        let size = size.max(1 << MIN_ORDER);
        let order = (size.next_power_of_two().trailing_zeros() as usize).saturating_sub(MIN_ORDER);
        if order > MAX_ORDER { None } else { Some(order) }
    }
}

// ── Trait ────────────────────────────────────────────────────────────────

/// Core buddy allocation interface.
///
/// Implement this on your own type (typically wrapping [`BuddyAllocator`])
/// and plug it into [`LockedBuddy`] to get a working `#[global_allocator]`.
pub trait BuddyAlloc {
    /// Initialise the allocator with the memory region `[base, base + size)`.
    fn init(&mut self, base: usize, size: usize);

    /// Allocate a block of `2^(order + MIN_ORDER)` bytes. Null on failure.
    fn alloc(&mut self, order: usize) -> *mut u8;

    /// Free a block of `2^(order + MIN_ORDER)` bytes at `ptr`.
    fn dealloc(&mut self, ptr: *mut u8, order: usize);
}

// ── GlobalAlloc bridge ──────────────────────────────────────────────────

/// Generic `GlobalAlloc` wrapper.
///
/// `T` must implement [`BuddyAlloc`] and be constructible as a `const`.
/// Declare as: `#[global_allocator] static HEAP: LockedBuddy<MyAlloc> = LockedBuddy::new(MyAlloc::new());`
pub struct LockedBuddy<T> {
    inner: UnsafeCell<T>,
}

// SAFETY: single-core kernel, no concurrent access.
unsafe impl<T> Sync for LockedBuddy<T> {}

impl<T> LockedBuddy<T> {
    /// Wrap an allocator instance.
    pub const fn new(inner: T) -> Self {
        Self {
            inner: UnsafeCell::new(inner),
        }
    }

    /// Get a mutable reference to the inner allocator.
    ///
    /// # Safety
    ///
    /// Caller must ensure exclusive access.
    #[inline]
    pub unsafe fn get_mut(&self) -> &mut T {
        &mut *self.inner.get()
    }
}

unsafe impl<T: BuddyAlloc> GlobalAlloc for LockedBuddy<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size().max(layout.align());
        match BuddyAllocator::size_to_order(size) {
            Some(order) => self.get_mut().alloc(order),
            None => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size().max(layout.align());
        if let Some(order) = BuddyAllocator::size_to_order(size) {
            self.get_mut().dealloc(ptr, order);
        }
    }
}
