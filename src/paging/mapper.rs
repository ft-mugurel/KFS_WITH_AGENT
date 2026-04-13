//! Page mapping functions.
//!
//! This module provides functions to:
//! - Map virtual addresses to physical addresses
//! - Unmap (invalidate) mappings
//! - Perform identity mapping (virtual = physical)
//! - Query physical addresses from virtual addresses

use super::page_directory::{PageDirectory, flags as pd_flags};
use super::page_table::{PageTable, flags as pt_flags};
use super::frame_allocator::get_frame_allocator;
use super::enable::invlpg;

/// Number of pages in 4MB (one page table)
pub const PAGES_PER_PT: usize = 1024;

/// Number of bytes in one 4KB page
pub const PAGE_SIZE: usize = 4096;

/// Number of bytes in 4MB (one page table covers this)
pub const BYTES_PER_PT: usize = PAGES_PER_PT * PAGE_SIZE;

/// Number of page tables needed to cover N bytes
pub const fn num_page_tables_for(bytes: usize) -> usize {
    (bytes + BYTES_PER_PT - 1) / BYTES_PER_PT
}

/// Global Page Directory - static allocation
/// This is placed in .bss and will be 4KB aligned due to #[repr(align(4096))]
static mut PAGE_DIRECTORY: PageDirectory = PageDirectory::new();

/// Global Page Tables for identity mapping
/// We allocate enough for 64MB of memory (16 page tables × 4MB each)
const NUM_IDENTITY_PTS: usize = 16; // 16 × 4MB = 64MB
static mut PAGE_TABLES: [PageTable; NUM_IDENTITY_PTS] = [PageTable::new(); NUM_IDENTITY_PTS];

/// Get a reference to the global Page Directory
pub fn get_page_directory() -> &'static mut PageDirectory {
    unsafe { &mut *(&raw mut PAGE_DIRECTORY) }
}

/// Perform identity mapping of the first 64MB of physical memory.
/// 
/// After this:
/// - Virtual address 0x00000000 → Physical address 0x00000000
/// - Virtual address 0x00100000 → Physical address 0x00100000 (kernel)
/// - ...etc
///
/// This is the simplest paging setup and allows the kernel to continue
/// running after paging is enabled.
pub unsafe fn identity_map() {
    let pd = get_page_directory();
    
    // Flags for kernel pages: Present + Writable (supervisor only by default)
    let pde_flags = pd_flags::PRESENT | pd_flags::WRITABLE;
    let pte_flags = pt_flags::PRESENT | pt_flags::WRITABLE;
    
    // For each of our 16 page tables (covering 64MB)
    for pt_idx in 0..NUM_IDENTITY_PTS {
        let pt = &mut PAGE_TABLES[pt_idx];
        
        // Fill each entry in this page table
        for page_idx in 0..PAGES_PER_PT {
            // Calculate physical address: 
            // pt_idx * 4MB + page_idx * 4KB
            let phys_addr = ((pt_idx * PAGES_PER_PT + page_idx) * PAGE_SIZE) as u32;
            
            pt.set_entry(page_idx, phys_addr, pte_flags);
        }
        
        // Point the corresponding PDE to this page table
        // PDE index = pt_idx (since each PT covers 4MB starting from 0)
        let pt_physical_addr = pt.as_ptr() as u32;
        pd.set_entry(pt_idx, pt_physical_addr, pde_flags);
    }
    
    // Note: We leave PDEs 16-1023 as 0 (not present).
    // Accessing those addresses will cause page faults.
}

/// Map a single virtual page to a physical page.
///
/// If the Page Directory entry for this virtual address does not yet have a
/// Page Table, one is **dynamically allocated** from the frame allocator
/// (zeroed, so every PTE starts as "not present").
///
/// After the mapping is installed the corresponding TLB entry is
/// invalidated so the CPU picks up the change immediately.
///
/// # Arguments
/// * `virt_addr` - Virtual address (aligned down to 4KB)
/// * `phys_addr` - Physical address (aligned down to 4KB)
/// * `writable`  - If true, the page is writable
/// * `user`      - If true, the page is accessible from user mode
///
/// # Safety
/// - The physical address must point to valid RAM.
/// - If the virtual address is already mapped the old mapping is silently
///   overwritten (the caller must have unmapped it first if the old frame
///   should be freed).
///
/// # Panics
/// Panics if the frame allocator is out of memory when a new Page Table is
/// needed.
pub unsafe fn map_page(virt_addr: u32, phys_addr: u32, writable: bool, user: bool) {
    let pd = get_page_directory();

    // Decompose the virtual address into PD / PT indices.
    let pd_index = (virt_addr as usize >> 22) & 0x3FF; // Bits 31:22
    let pt_index = (virt_addr as usize >> 12) & 0x3FF; // Bits 21:12

    // Align the physical address to a 4 KB boundary.
    let phys_aligned = phys_addr & 0xFFFFF000;

    // ── PTE flags ──────────────────────────────────────────────────────
    let mut pte_flags = pt_flags::PRESENT;
    if writable { pte_flags |= pt_flags::WRITABLE; }
    if user     { pte_flags |= pt_flags::USER; }

    // ── PDE flags ──────────────────────────────────────────────────────
    // The PDE must be at least PRESENT | WRITABLE (so the CPU can walk
    // through to the PTE).  If we are mapping a user-accessible page the
    // PDE must *also* carry USER — otherwise the CPU denies access at the
    // directory level before it ever looks at the PTE.
    let mut pde_flags = pd_flags::PRESENT | pd_flags::WRITABLE;
    if user { pde_flags |= pd_flags::USER; }

    // ── Ensure a Page Table exists for this PDE ────────────────────────
    if !pd.is_present(pd_index) {
        // Allocate a fresh, zeroed 4 KB frame for the new Page Table.
        let new_pt_phys = get_frame_allocator()
            .alloc_frame_zeroed()
            .expect("map_page: out of memory — cannot allocate Page Table");

        pd.set_entry(pd_index, new_pt_phys, pde_flags);
    } else if user {
        // The PDE already exists but may lack the USER bit.  Promote it
        // so user-mode pages in this table are reachable.
        let existing = pd.get_entry(pd_index);
        if existing & pd_flags::USER == 0 {
            pd.entries[pd_index] = existing | pd_flags::USER;
        }
    }

    // ── Write the PTE ──────────────────────────────────────────────────
    let pt_addr = pd.get_pt_address(pd_index).unwrap();
    let pt = &mut *(pt_addr as *mut PageTable);
    pt.set_entry(pt_index, phys_aligned, pte_flags);

    // ── Invalidate the TLB for this virtual address ────────────────────
    invlpg(virt_addr);
}

/// Unmap a virtual page (mark as not present) and invalidate its TLB entry.
///
/// Does nothing if the page directory entry is not present.
///
/// If, after clearing the PTE, the entire page table is empty **and** it was
/// dynamically allocated (i.e. not one of the static identity-map tables for
/// PDE indices 0..NUM_IDENTITY_PTS), the page table frame is returned to the
/// frame allocator and the PDE is cleared.  This prevents a slow leak of
/// page-table frames when regions are repeatedly mapped and unmapped.
///
/// **Note:** this does *not* free the physical frame that was mapped — the
/// caller is responsible for returning it to the frame allocator if needed.
///
/// # Arguments
/// * `virt_addr` - Virtual address to unmap (aligned to 4KB)
pub unsafe fn unmap_page(virt_addr: u32) {
    let pd = get_page_directory();

    let pd_index = (virt_addr as usize >> 22) & 0x3FF;
    let pt_index = (virt_addr as usize >> 12) & 0x3FF;

    if !pd.is_present(pd_index) {
        return; // Nothing to unmap
    }

    let pt_addr = pd.get_pt_address(pd_index).unwrap();
    let pt = &mut *(pt_addr as *mut PageTable);

    pt.clear_entry(pt_index);

    // Tell the CPU to drop any cached translation for this address.
    invlpg(virt_addr);

    // --- Reclaim empty, dynamically-allocated page tables ------------------
    // The first NUM_IDENTITY_PTS page tables live in the static PAGE_TABLES
    // array (.bss) and must never be freed.  Everything from index
    // NUM_IDENTITY_PTS onward was allocated from the frame allocator.
    if pd_index >= NUM_IDENTITY_PTS && is_page_table_empty(pt) {
        // Return the page-table frame to the allocator.
        get_frame_allocator().free_frame(pt_addr);
        // Clear the PDE so future accesses fault cleanly.
        pd.clear_entry(pd_index);
    }
}

/// Check whether every entry in a page table is zero (not present).
fn is_page_table_empty(pt: &PageTable) -> bool {
    for i in 0..PAGES_PER_PT {
        if pt.entries[i] != 0 {
            return false;
        }
    }
    true
}

/// Translate a virtual address to its physical address.
/// 
/// Returns None if the page is not mapped.
pub fn get_physical_address(virt_addr: u32) -> Option<u32> {
    let pd = get_page_directory();
    
    let pd_index = (virt_addr as usize >> 22) & 0x3FF;
    let pt_index = (virt_addr as usize >> 12) & 0x3FF;
    let offset = virt_addr & 0xFFF;
    
    if !pd.is_present(pd_index) {
        return None;
    }
    
    let pt_addr = pd.get_pt_address(pd_index)?;
    let pt = unsafe { &*(pt_addr as *const PageTable) };
    
    if !pt.is_present(pt_index) {
        return None;
    }
    
    let page_addr = pt.get_page_address(pt_index)?;
    Some(page_addr | offset)
}

/// Get a mutable reference to a specific page table (by index 0-1023).
/// 
/// # Safety
/// Only use this for page tables that have been properly set up.
pub unsafe fn get_page_table(pd_index: usize) -> Option<&'static mut PageTable> {
    let pd = get_page_directory();
    
    if !pd.is_present(pd_index) {
        None
    } else {
        let pt_addr = pd.get_pt_address(pd_index)?;
        Some(&mut *(pt_addr as *mut PageTable))
    }
}

// ---------------------------------------------------------------------------
// Range helpers
// ---------------------------------------------------------------------------

/// Map a contiguous range of virtual pages to a contiguous range of physical
/// pages.
///
/// Both `virt_start` and `phys_start` are aligned **down** to 4 KB.
/// `size` is rounded **up** to the next 4 KB boundary so partial pages at
/// the end are included.
///
/// # Arguments
/// * `virt_start` - Starting virtual address
/// * `phys_start` - Starting physical address
/// * `size`       - Number of bytes to map (rounded up to 4 KB)
/// * `writable`   - If true, pages are writable
/// * `user`       - If true, pages are user-accessible
///
/// # Safety
/// Same requirements as [`map_page`].
pub unsafe fn map_range(
    virt_start: u32,
    phys_start: u32,
    size: usize,
    writable: bool,
    user: bool,
) {
    let virt_base = virt_start & 0xFFFFF000;
    let phys_base = phys_start & 0xFFFFF000;
    let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

    for i in 0..num_pages {
        let offset = (i * PAGE_SIZE) as u32;
        map_page(virt_base + offset, phys_base + offset, writable, user);
    }
}

/// Unmap a contiguous range of virtual pages.
///
/// `virt_start` is aligned down to 4 KB and `size` is rounded up.
///
/// **Note:** the underlying physical frames are **not** freed — the caller
/// must return them to the frame allocator separately if required.
///
/// # Arguments
/// * `virt_start` - Starting virtual address
/// * `size`       - Number of bytes to unmap (rounded up to 4 KB)
///
/// # Safety
/// Caller must ensure no live references depend on the unmapped pages.
pub unsafe fn unmap_range(virt_start: u32, size: usize) {
    let virt_base = virt_start & 0xFFFFF000;
    let num_pages = (size + PAGE_SIZE - 1) / PAGE_SIZE;

    for i in 0..num_pages {
        let offset = (i * PAGE_SIZE) as u32;
        unmap_page(virt_base + offset);
    }
}
