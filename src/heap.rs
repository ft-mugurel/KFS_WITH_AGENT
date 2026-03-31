//! Kernel Heap Allocator — bump allocator backed by paged virtual memory.
//!
//! The heap occupies a dedicated virtual region **above** the 64 MB identity
//! map.  At init time we allocate physical frames via the frame allocator and
//! map them into this region with [`map_page`].  A simple bump allocator then
//! hands out bytes from that region.
//!
//! Because the allocator implements [`GlobalAlloc`] and is registered with
//! `#[global_allocator]`, the standard `alloc` crate (`Box`, `Vec`, …)
//! works kernel-wide after [`init`] has been called.

use core::alloc::{GlobalAlloc, Layout};
use crate::paging::mapper::PAGE_SIZE;
use crate::paging::{map_page, get_frame_allocator};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Virtual start of the kernel heap (just past the 64 MB identity map).
pub const HEAP_START: usize = 0x0400_0000;

/// Initial heap size in bytes (64 KB = 16 pages).
pub const HEAP_INIT_SIZE: usize = 64 * 1024;

// ---------------------------------------------------------------------------
// Heap state (internal)
// ---------------------------------------------------------------------------

/// Tracks where the next allocation will come from.
struct HeapState {
    /// Next free byte in the heap region.
    next: usize,
    /// One-past-the-end of the mapped heap region.
    end: usize,
}

/// Global heap state — lives in `.bss`.
static mut HEAP_STATE: HeapState = HeapState { next: 0, end: 0 };

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Map physical frames into the heap region and mark the allocator as ready.
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

        // Zero the frame through its *virtual* address (identity mapping
        // is still active for the physical range, but we write through the
        // heap virtual address to validate the mapping works).
        core::ptr::write_bytes(virt as *mut u8, 0, PAGE_SIZE);
    }

    let state = &mut *(&raw mut HEAP_STATE);
    state.next = HEAP_START;
    state.end = HEAP_START + HEAP_INIT_SIZE;
}

// ---------------------------------------------------------------------------
// Diagnostic helpers (used by shell commands)
// ---------------------------------------------------------------------------

/// Number of heap bytes that have been handed out.
pub fn used() -> usize {
    let state = unsafe { &*(&raw const HEAP_STATE) };
    state.next.saturating_sub(HEAP_START)
}

/// Number of heap bytes still available.
pub fn free() -> usize {
    let state = unsafe { &*(&raw const HEAP_STATE) };
    state.end.saturating_sub(state.next)
}

/// Total size of the mapped heap region.
pub fn total() -> usize {
    let state = unsafe { &*(&raw const HEAP_STATE) };
    state.end.saturating_sub(HEAP_START)
}

// ---------------------------------------------------------------------------
// GlobalAlloc implementation (bump allocator)
// ---------------------------------------------------------------------------

/// Zero-sized type — all state lives in [`HEAP_STATE`].
struct KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let state = &mut *(&raw mut HEAP_STATE);

        // Round `next` up to the required alignment.
        let aligned = (state.next + layout.align() - 1) & !(layout.align() - 1);
        let new_next = aligned + layout.size();

        if new_next > state.end {
            // Out of heap space.
            return core::ptr::null_mut();
        }

        state.next = new_next;
        aligned as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator: individual frees are no-ops.
        // Memory is only reclaimed when the entire heap is reset.
    }
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator;
