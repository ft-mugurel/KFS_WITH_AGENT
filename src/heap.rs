//! Kernel Heap Allocator -- free-list allocator backed by paged virtual memory.
//!
//! The heap occupies a dedicated virtual region **above** the 64 MB identity
//! map.  At init time we allocate physical frames via the frame allocator and
//! map them into this region with [`map_page`].
//!
//! The allocator maintains an intrusive linked list of free blocks.  Each free
//! block carries a header (`FreeBlock`) embedded at its start:
//!
//! ```text
//!   +--------+--------+----- ... -----+
//!   |  size  |  next  |  unused bytes |
//!   +--------+--------+----- ... -----+
//!   <-------- size bytes total -------->
//! ```
//!
//! ## Allocation strategy
//!
//! 1. Walk the free list looking for the **first block** that is large enough
//!    (first-fit).
//! 2. If the remainder after carving out the requested region is large enough
//!    to hold another `FreeBlock` header, **split** the block and keep the
//!    leftover on the free list.
//! 3. If no block is large enough, **grow** the heap by mapping fresh physical
//!    frames and append a new free block at the top.
//!
//! ## Deallocation
//!
//! The freed region is inserted back into the free list (sorted by address)
//! and then **coalesced** with any adjacent neighbours to avoid fragmentation.
//!
//! Because the allocator implements [`GlobalAlloc`] and is registered with
//! `#[global_allocator]`, the standard `alloc` crate (`Box`, `Vec`, ...)
//! works kernel-wide after [`init`] has been called.

use core::alloc::{GlobalAlloc, Layout};
use crate::paging::mapper::PAGE_SIZE;
use crate::paging::{map_page, get_frame_allocator};
use crate::paging::vma;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Virtual start of the kernel heap (just past the 64 MB identity map).
pub const HEAP_START: usize = 0x0400_0000;

/// Initial heap size in bytes (64 KB = 16 pages).
pub const HEAP_INIT_SIZE: usize = 64 * 1024;

/// Maximum the heap is allowed to grow to (4 MB).
/// This caps how much physical memory the heap can consume.
const HEAP_MAX_SIZE: usize = 4 * 1024 * 1024;

/// Minimum block size.  A free block must be at least this big so that the
/// `FreeBlock` header fits and we have room to split blocks.
const MIN_BLOCK_SIZE: usize = core::mem::size_of::<FreeBlock>();

// ---------------------------------------------------------------------------
// Free-block header (intrusive linked list node)
// ---------------------------------------------------------------------------

/// Header stored at the start of every free block.
///
/// When a block is *allocated* this header is overwritten by the caller's
/// data -- that is fine because we only read the header while the block is
/// on the free list.
struct FreeBlock {
    /// Total size of this free block **including** the header.
    size: usize,
    /// Pointer to the next free block, or null if this is the last one.
    next: *mut FreeBlock,
}

// ---------------------------------------------------------------------------
// Heap state
// ---------------------------------------------------------------------------

/// All mutable state for the heap allocator.
struct HeapState {
    /// Head of the free list (null before `init`).
    free_list: *mut FreeBlock,
    /// One-past-the-end of the currently *mapped* heap region.
    mapped_end: usize,
    /// Running total of bytes handed out (net of frees).
    allocated_bytes: usize,
}

/// Global heap state -- lives in `.bss`.
static mut HEAP_STATE: HeapState = HeapState {
    free_list: core::ptr::null_mut(),
    mapped_end: 0,
    allocated_bytes: 0,
};

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Map physical frames into the heap region and seed the free list.
///
/// Must be called **after** paging and the frame allocator have been
/// initialised, and **before** any `alloc` crate usage (`Box`, `Vec`, etc.).
///
/// # Panics
/// Panics if the frame allocator cannot supply enough frames.
pub unsafe fn init() {
    let fa = get_frame_allocator();
    let num_pages = HEAP_INIT_SIZE / PAGE_SIZE;

    for i in 0..num_pages {
        let virt = (HEAP_START + i * PAGE_SIZE) as u32;
        let frame = fa
            .alloc_frame()
            .expect("heap::init: out of physical frames");

        // Map virtual heap page -> physical frame (writable, supervisor only).
        map_page(virt, frame, true, false);

        // Zero the frame through its *virtual* address.
        core::ptr::write_bytes(virt as *mut u8, 0, PAGE_SIZE);
    }

    // Seed the free list with one giant block spanning the whole region.
    let first_block = HEAP_START as *mut FreeBlock;
    (*first_block).size = HEAP_INIT_SIZE;
    (*first_block).next = core::ptr::null_mut();

    let state = &mut *(&raw mut HEAP_STATE);
    state.free_list = first_block;
    state.mapped_end = HEAP_START + HEAP_INIT_SIZE;
    state.allocated_bytes = 0;
}

// ---------------------------------------------------------------------------
// Diagnostic helpers (used by shell commands)
// ---------------------------------------------------------------------------

/// Number of heap bytes that have been handed out (net of frees).
pub fn used() -> usize {
    let state = unsafe { &*(&raw const HEAP_STATE) };
    state.allocated_bytes
}

/// Number of heap bytes sitting in the free list.
pub fn free() -> usize {
    unsafe {
        let state = &*(&raw const HEAP_STATE);
        let mut total_free: usize = 0;
        let mut cursor = state.free_list;
        while !cursor.is_null() {
            total_free += (*cursor).size;
            cursor = (*cursor).next;
        }
        total_free
    }
}

/// Total size of the mapped heap region.
pub fn total() -> usize {
    let state = unsafe { &*(&raw const HEAP_STATE) };
    state.mapped_end.saturating_sub(HEAP_START)
}

// ---------------------------------------------------------------------------
// Heap growth
// ---------------------------------------------------------------------------

/// Extend the mapped heap region by `extra_bytes` (rounded up to PAGE_SIZE).
///
/// New pages are allocated from the frame allocator and mapped contiguously
/// at the current `mapped_end`.  A single free block covering the entire
/// extension is inserted into the free list.
///
/// Returns `true` on success, `false` if we hit HEAP_MAX_SIZE or run out of
/// physical frames.
unsafe fn grow_heap(extra_bytes: usize) -> bool {
    let state = &mut *(&raw mut HEAP_STATE);

    // Round up to whole pages.
    let pages_needed = (extra_bytes + PAGE_SIZE - 1) / PAGE_SIZE;
    let grow_bytes = pages_needed * PAGE_SIZE;

    // Enforce maximum heap size.
    let new_end = state.mapped_end + grow_bytes;
    if new_end - HEAP_START > HEAP_MAX_SIZE {
        return false;
    }

    let fa = get_frame_allocator();

    for i in 0..pages_needed {
        let virt = (state.mapped_end + i * PAGE_SIZE) as u32;
        let frame = match fa.alloc_frame() {
            Some(f) => f,
            None => return false,
        };
        map_page(virt, frame, true, false);
        core::ptr::write_bytes(virt as *mut u8, 0, PAGE_SIZE);
    }

    // Create a free block spanning the newly mapped region and add it to
    // the free list.  `free_list_insert` will coalesce it with any
    // adjacent block that sits right at the old mapped_end.
    let new_block = state.mapped_end as *mut FreeBlock;
    (*new_block).size = grow_bytes;
    (*new_block).next = core::ptr::null_mut();
    free_list_insert(new_block);

    state.mapped_end = new_end;

    // Keep the VMA table in sync so the page-fault handler and
    // diagnostics reflect the new heap size.
    vma::extend(HEAP_START as u32, new_end as u32);

    true
}

// ---------------------------------------------------------------------------
// Free-list helpers
// ---------------------------------------------------------------------------

/// Insert `block` into the free list in address order and coalesce with
/// immediate neighbours if they are physically adjacent.
unsafe fn free_list_insert(block: *mut FreeBlock) {
    let state = &mut *(&raw mut HEAP_STATE);

    let block_addr = block as usize;

    // Find the insertion point: we want `prev` to be the last block whose
    // address is less than `block`, so the list stays sorted by address.
    let mut prev: *mut FreeBlock = core::ptr::null_mut();
    let mut cursor = state.free_list;

    while !cursor.is_null() && (cursor as usize) < block_addr {
        prev = cursor;
        cursor = (*cursor).next;
    }

    // `block` goes between `prev` and `cursor`.
    // Wire up the forward pointer first (block -> cursor).
    (*block).next = cursor;

    if prev.is_null() {
        // `block` becomes the new head of the list.
        state.free_list = block;
    } else {
        (*prev).next = block;
    }

    // --- Coalesce with the *next* block -----------------------------------
    if !cursor.is_null() {
        let block_end = block_addr + (*block).size;
        if block_end == cursor as usize {
            // `block` and `cursor` are contiguous -- merge.
            (*block).size += (*cursor).size;
            (*block).next = (*cursor).next;
        }
    }

    // --- Coalesce with the *previous* block --------------------------------
    if !prev.is_null() {
        let prev_end = prev as usize + (*prev).size;
        if prev_end == block_addr {
            // `prev` and `block` are contiguous -- merge.
            (*prev).size += (*block).size;
            (*prev).next = (*block).next;
        }
    }
}

// ---------------------------------------------------------------------------
// Core alloc / dealloc
// ---------------------------------------------------------------------------

/// Align `addr` upward to the next multiple of `align` (must be power of 2).
#[inline]
fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

/// Try to allocate `layout.size()` bytes with `layout.align()` alignment
/// from the free list.  Returns a pointer on success, null on failure.
///
/// Strategy: first-fit with splitting.
unsafe fn alloc_inner(layout: Layout) -> *mut u8 {
    let state = &mut *(&raw mut HEAP_STATE);

    let alloc_size = layout.size().max(MIN_BLOCK_SIZE);
    let alloc_align = layout.align();

    let mut prev: *mut FreeBlock = core::ptr::null_mut();
    let mut cursor = state.free_list;

    while !cursor.is_null() {
        let block_start = cursor as usize;
        let block_size = (*cursor).size;
        let _block_end = block_start + block_size;

        // Where the payload would start after aligning.
        let aligned_start = align_up(block_start, alloc_align);
        // How much space we lose to alignment padding.
        let padding = aligned_start - block_start;
        // Total space this block needs: padding + requested bytes.
        let needed = padding + alloc_size;

        if block_size >= needed {
            // --- This block is big enough ---

            // If there is alignment padding that is large enough to hold
            // its own FreeBlock, split the padding off as a separate free
            // block so we don't waste it.
            if padding >= MIN_BLOCK_SIZE {
                // Keep the front part as a free block of size `padding`.
                let front = cursor;
                (*front).size = padding;
                // `front.next` will be fixed up below.

                // The remainder is our allocation candidate.
                let new_cursor = aligned_start as *mut FreeBlock;
                (*new_cursor).size = block_size - padding;
                (*new_cursor).next = (*front).next;

                // Splice: front -> new_cursor -> old_next
                (*front).next = new_cursor;

                // Now try to split/allocate from `new_cursor`.
                prev = front;
                cursor = new_cursor;
                // Fall through -- cursor is now perfectly aligned.
            }

            let cursor_size = (*cursor).size;

            // Can we split the (possibly updated) block?
            let remainder = cursor_size - alloc_size;
            if remainder >= MIN_BLOCK_SIZE {
                // Create a new free block for the leftover.
                let split = (cursor as usize + alloc_size) as *mut FreeBlock;
                (*split).size = remainder;
                (*split).next = (*cursor).next;

                // Remove the front `alloc_size` portion from the list.
                if prev.is_null() {
                    state.free_list = split;
                } else {
                    (*prev).next = split;
                }
            } else {
                // Use the whole block (no split -- too small).
                // Just unlink `cursor`.
                if prev.is_null() {
                    state.free_list = (*cursor).next;
                } else {
                    (*prev).next = (*cursor).next;
                }
            }

            state.allocated_bytes += alloc_size;
            return cursor as *mut u8;
        }

        prev = cursor;
        cursor = (*cursor).next;
    }

    // No suitable block found.
    core::ptr::null_mut()
}

// ---------------------------------------------------------------------------
// GlobalAlloc implementation
// ---------------------------------------------------------------------------

/// Zero-sized type -- all state lives in [`HEAP_STATE`].
struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // First attempt.
        let ptr = alloc_inner(layout);
        if !ptr.is_null() {
            return ptr;
        }

        // Not enough space -- try to grow the heap and retry once.
        let needed = layout.size().max(MIN_BLOCK_SIZE) + layout.align();
        // Grow by at least the amount we need, or 64 KB, whichever is larger.
        // This avoids growing page-by-page for many small allocations.
        let grow_by = needed.max(64 * 1024);
        if !grow_heap(grow_by) {
            return core::ptr::null_mut();
        }
        alloc_inner(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let state = &mut *(&raw mut HEAP_STATE);

        let alloc_size = layout.size().max(MIN_BLOCK_SIZE);

        // Turn the freed region into a FreeBlock and insert it back into
        // the sorted free list with coalescing.
        let block = ptr as *mut FreeBlock;
        (*block).size = alloc_size;
        (*block).next = core::ptr::null_mut();
        free_list_insert(block);

        state.allocated_bytes = state.allocated_bytes.saturating_sub(alloc_size);
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;
