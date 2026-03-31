//! Paging module for x86 32-bit virtual memory management.
//!
//! This module implements:
//! - Page Directory (PD) and Page Table (PT) structures
//! - Page mapping/unmapping functions
//! - Identity mapping (virtual = physical)
//! - Physical frame allocator (bitmap-based)
//! - Enabling paging via CR0/CR3

pub mod page_directory;
pub mod page_table;
pub mod mapper;
pub mod enable;
pub mod frame_allocator;

pub use page_directory::PageDirectory;
pub use page_table::PageTable;
pub use mapper::{identity_map, map_page, unmap_page, map_range, unmap_range, get_physical_address};
pub use enable::{enable_paging, flush_tlb, disable_paging, is_paging_enabled};
pub use frame_allocator::get_frame_allocator;

// Linker symbol: first address past the entire kernel image.
extern "C" {
    static _kernel_end: u8;
}

/// Initialize paging: identity map, frame allocator, then enable CR0.PG.
///
/// # Boot sequence
/// 1. **Identity map** the first 64 MB (virtual = physical).
/// 2. **Frame allocator** — mark kernel region as used, rest as free.
/// 3. **Enable paging** (load CR3 + set CR0.PG).
///
/// Must be called before any code that depends on dynamic page allocation.
pub unsafe fn init() {
    // Step 1 — build the identity-mapped page tables.
    identity_map();

    // Step 2 — initialise the physical frame allocator.
    //   Everything from physical 0 up to _kernel_end is marked *used*.
    //   Everything from _kernel_end up to 64 MB is marked *free*.
    let kernel_end = &_kernel_end as *const u8 as usize;
    frame_allocator::get_frame_allocator().init(
        frame_allocator::MAX_MEMORY, // 64 MB
        kernel_end,
    );

    // Step 3 — flip the paging bit in CR0.
    enable_paging();
}
