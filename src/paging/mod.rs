//! Paging module for x86 32-bit virtual memory management.
//!
//! This module implements:
//! - Page Directory (PD) and Page Table (PT) structures
//! - Page mapping/unmapping functions
//! - Identity mapping (virtual = physical)
//! - Physical frame allocator (bitmap-based)
//! - Enabling paging via CR0/CR3
//! - Virtual Memory Area (VMA) tracking

pub mod page_directory;
pub mod page_table;
pub mod mapper;
pub mod enable;
pub mod frame_allocator;
pub mod vma;

pub use page_directory::PageDirectory;
pub use page_table::PageTable;
pub use mapper::{identity_map, map_page, unmap_page, map_range, unmap_range, get_physical_address};
pub use enable::{enable_paging, flush_tlb, disable_paging, is_paging_enabled};
pub use frame_allocator::get_frame_allocator;

use vma::{VmaKind, VmaFlags};

// Linker symbols.
extern "C" {
    static _kernel_end: u8;
    static kernel_stack_bottom: u8;
    static kernel_stack_top: u8;
    static user_stack_bottom: u8;
    static user_stack_top: u8;
    static kernel_guard_page: u8;
    static user_guard_page: u8;
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
    // Step 1 -- build the identity-mapped page tables.
    identity_map();

    // Step 2 -- initialise the physical frame allocator.
    //   Everything from physical 0 up to _kernel_end is marked *used*.
    //   Everything from _kernel_end up to 64 MB is marked *free*.
    let kernel_end = &_kernel_end as *const u8 as usize;
    frame_allocator::get_frame_allocator().init(
        frame_allocator::MAX_MEMORY, // 64 MB
        kernel_end,
    );

    // Step 3 -- flip the paging bit in CR0.
    enable_paging();

    // Step 4 -- register all known virtual memory regions in the VMA table.
    register_boot_vmas();

    // Step 5 -- unmap guard pages so they trigger page faults on access.
    setup_guard_pages();
}

/// Register every known virtual memory region at boot time.
///
/// This populates the VMA table so the page-fault handler (and diagnostic
/// commands) know what each address range is for.
unsafe fn register_boot_vmas() {
    let rw  = VmaFlags::new(VmaFlags::READ | VmaFlags::WRITE);
    let rwx = VmaFlags::new(VmaFlags::READ | VmaFlags::WRITE | VmaFlags::EXEC);

    // 1. Identity map (0 .. 64 MB) -- contains BIOS, VGA, kernel, free frames.
    vma::register(
        0x0000_0000,
        frame_allocator::MAX_MEMORY as u32,
        VmaKind::Identity,
        rwx,
    );

    // 2. VGA text framebuffer -- sits inside the identity map but we call
    //    it out as MMIO so diagnostics can show it separately.
    vma::register(
        0x000B_8000,
        0x000B_8000 + mapper::PAGE_SIZE as u32,
        VmaKind::Mmio,
        rw,
    );

    // 3. Kernel image (1 MB .. _kernel_end).
    vma::register(
        0x0010_0000,
        &_kernel_end as *const u8 as u32,
        VmaKind::KernelImage,
        rwx,
    );

    // 4. Kernel stack guard page.
    let kguard = &kernel_guard_page as *const u8 as u32;
    vma::register(
        kguard,
        kguard + mapper::PAGE_SIZE as u32,
        VmaKind::Guard,
        VmaFlags::new(VmaFlags::NONE), // no access allowed
    );

    // 5. Kernel stack.
    let kbot = &kernel_stack_bottom as *const u8 as u32;
    let ktop = &kernel_stack_top as *const u8 as u32;
    vma::register(kbot, ktop, VmaKind::KernelStack, rw);

    // 6. User stack guard page.
    let uguard = &user_guard_page as *const u8 as u32;
    vma::register(
        uguard,
        uguard + mapper::PAGE_SIZE as u32,
        VmaKind::Guard,
        VmaFlags::new(VmaFlags::NONE),
    );

    // 7. User stack.
    let ubot = &user_stack_bottom as *const u8 as u32;
    let utop = &user_stack_top as *const u8 as u32;
    vma::register(
        ubot, utop, VmaKind::UserStack,
        VmaFlags::new(VmaFlags::READ | VmaFlags::WRITE | VmaFlags::USER),
    );

    // 8. Kernel heap -- starts at HEAP_START, initial size will be set by
    //    heap::init().  We register it here with DEMAND flag so the page-
    //    fault handler knows this region is valid for demand paging.
    vma::register(
        crate::heap::HEAP_START as u32,
        (crate::heap::HEAP_START + crate::heap::HEAP_INIT_SIZE) as u32,
        VmaKind::Heap,
        VmaFlags::new(VmaFlags::READ | VmaFlags::WRITE | VmaFlags::DEMAND),
    );
}

/// Unmap the guard pages that sit just below each stack.
///
/// These pages are part of the identity map (their physical frames are in
/// the first 64 MB and were mapped by `identity_map()`).  By unmapping
/// them we create a deliberate hole: if the stack grows past its bottom
/// boundary, the CPU will hit the guard page, trigger a page fault, and
/// the handler will print "stack overflow" instead of silently corrupting
/// whatever sits below the stack.
unsafe fn setup_guard_pages() {
    let kguard = &kernel_guard_page as *const u8 as u32;
    unmap_page(kguard);

    let uguard = &user_guard_page as *const u8 as u32;
    unmap_page(uguard);
}
