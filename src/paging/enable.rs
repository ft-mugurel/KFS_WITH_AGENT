//! Paging enable/disable and TLB management.
//!
//! This module handles:
//! - Loading CR3 with Page Directory address
//! - Enabling paging via CR0.PG bit
//! - Disabling paging
//! - TLB (Translation Lookaside Buffer) flushing

use core::arch::asm;

/// Check if paging is currently enabled (CR0.PG bit)
pub fn is_paging_enabled() -> bool {
    let cr0: u32;
    unsafe {
        asm!("mov {0}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    }
    (cr0 & (1 << 31)) != 0
}

/// Load a new Page Directory into CR3.
/// 
/// # Arguments
/// * `pd_physical_addr` - Physical address of the Page Directory (must be 4KB aligned)
///
/// # Safety
/// - The address must point to a valid 4KB-aligned Page Directory
/// - Before paging is enabled, virtual = physical, so any address works
/// - After paging is enabled, this must be a valid physical address in mapped memory
pub unsafe fn load_cr3(pd_physical_addr: u32) {
    debug_assert!(pd_physical_addr & 0xFFF == 0, "Page Directory must be 4KB aligned");
    
    asm!(
        "mov cr3, {0}",
        in(reg) pd_physical_addr,
        options(nomem, nostack, preserves_flags)
    );
}

/// Get the current CR3 value (physical address of Page Directory)
pub fn get_cr3() -> u32 {
    let cr3: u32;
    unsafe {
        asm!("mov {0}, cr3", out(reg) cr3, options(nomem, nostack, preserves_flags));
    }
    cr3
}

/// Enable paging by setting CR0.PG (bit 31).
/// 
/// This must be called after:
/// 1. A valid Page Directory is set up
/// 2. CR3 is loaded with the PD address
///
/// # Safety
/// - All memory the CPU will access must be mapped (kernel code, stack, GDT, IDT, etc.)
/// - If not, you'll get an immediate page fault → triple fault → reboot
pub unsafe fn enable_paging() {
    // Load CR3 with our Page Directory
    let pd = super::mapper::get_page_directory();
    let pd_addr = pd.physical_address();
    load_cr3(pd_addr);
    
    // Read CR0, set PG bit (31), write back
    let cr0: u32;
    asm!("mov {0}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    
    let new_cr0 = cr0 | (1 << 31); // Set PG bit
    
    asm!("mov cr0, {0}", in(reg) new_cr0, options(nomem, nostack, preserves_flags));
}

/// Disable paging by clearing CR0.PG (bit 31).
/// 
/// # Safety
/// After disabling, all memory accesses use physical addresses directly.
pub unsafe fn disable_paging() {
    let cr0: u32;
    asm!("mov {0}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
    
    let new_cr0 = cr0 & !(1 << 31); // Clear PG bit
    
    asm!("mov cr0, {0}", in(reg) new_cr0, options(nomem, nostack, preserves_flags));
}

/// Flush the entire TLB by reloading CR3.
/// 
/// The TLB (Translation Lookaside Buffer) caches virtual→physical translations.
/// After modifying page tables, the TLB may have stale entries.
/// Reloading CR3 forces the CPU to flush all TLB entries.
pub unsafe fn flush_tlb() {
    let cr3 = get_cr3();
    load_cr3(cr3);
}

/// Invalidate a single TLB entry for the given virtual address.
/// 
/// This is more efficient than flushing the entire TLB when you only
/// modified one page's mapping.
/// 
/// # Arguments
/// * `virt_addr` - Virtual address whose TLB entry should be invalidated
pub unsafe fn invlpg(virt_addr: u32) {
    asm!(
        "invlpg [{0}]",
        in(reg) virt_addr,
        options(nomem, nostack, preserves_flags)
    );
}

/// Enable paging with the given Page Directory address.
/// This is a convenience function that loads CR3 and enables CR0.PG.
pub unsafe fn enable_paging_with_cr3(pd_physical_addr: u32) {
    load_cr3(pd_physical_addr);
    enable_paging();
}
