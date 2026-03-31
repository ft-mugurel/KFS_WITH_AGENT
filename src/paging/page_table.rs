//! Page Table implementation for x86 32-bit paging.
//!
//! A Page Table is a 4KB page containing 1024 Page Table Entries (PTEs).
//! Each PTE points to a 4KB physical page of memory.
//!
//! Layout:
//! - 1024 PTEs × 4 bytes = 4096 bytes (4KB)
//! - Each PTE covers 4KB of virtual address space
//! - Total coverage per Page Table: 1024 × 4KB = 4MB

/// Flags for Page Table Entries (same as PDE flags, plus a few more)
pub mod flags {
    /// Page is present in memory
    pub const PRESENT: u32 = 1 << 0;
    /// Page is writable (else read-only)
    pub const WRITABLE: u32 = 1 << 1;
    /// Page is accessible from user mode (else supervisor only)
    pub const USER: u32 = 1 << 2;
    /// Page uses write-through caching (else write-back)
    pub const WRITE_THROUGH: u32 = 1 << 3;
    /// Page caching is disabled
    pub const CACHE_DISABLED: u32 = 1 << 4;
    /// Page has been accessed (set by CPU on read/write)
    pub const ACCESSED: u32 = 1 << 5;
    /// Page has been written to / dirty (set by CPU on write, PTE only)
    pub const DIRTY: u32 = 1 << 6;
    /// Page size: 0 = 4KB (PTE), 1 = 4MB (PDE with CR4.PSE)
    pub const PAGE_SIZE: u32 = 1 << 7;
    /// Global page - not flushed from TLB on CR3 change (if CR4.PGE=1)
    pub const GLOBAL: u32 = 1 << 8;
}

/// A Page Table Entry (PTE) is a 32-bit value containing:
/// - Bits 31:12 = Physical address of the 4KB page (must be 4KB aligned)
/// - Bits 11:0  = Flags (present, writable, user, accessed, dirty, etc.)
pub type PageTableEntry = u32;

/// A Page Table - must be 4KB aligned.
/// Contains 1024 PTEs, each covering a 4KB physical page.
/// One Page Table covers 1024 × 4KB = 4MB of virtual address space.
#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct PageTable {
    /// Array of 1024 PTEs
    pub entries: [PageTableEntry; 1024],
}

impl PageTable {
    /// Create a new empty Page Table (all entries = 0 = not present)
    pub const fn new() -> Self {
        PageTable {
            entries: [0; 1024],
        }
    }

    /// Set a Page Table Entry.
    ///
    /// # Arguments
    /// * `index` - PTE index (0-1023)
    /// * `page_physical_addr` - Physical address of the 4KB page (must be 4KB aligned)
    /// * `flags` - Combination of flags:: constants
    pub fn set_entry(&mut self, index: usize, page_physical_addr: u32, flags: u32) {
        debug_assert!(page_physical_addr & 0xFFF == 0, "Page address must be 4KB aligned");
        debug_assert!(index < 1024, "PTE index out of range");
        
        // PTE format: upper 20 bits = address, lower 12 bits = flags
        self.entries[index] = (page_physical_addr & 0xFFFFF000) | (flags & 0xFFF);
    }

    /// Clear a PTE (mark as not present)
    pub fn clear_entry(&mut self, index: usize) {
        debug_assert!(index < 1024, "PTE index out of range");
        self.entries[index] = 0;
    }

    /// Get the raw PTE value at the given index
    pub fn get_entry(&self, index: usize) -> PageTableEntry {
        debug_assert!(index < 1024, "PTE index out of range");
        self.entries[index]
    }

    /// Check if a PTE is present
    pub fn is_present(&self, index: usize) -> bool {
        (self.entries[index] & flags::PRESENT) != 0
    }

    /// Get the physical address this PTE points to
    /// Returns None if not present
    pub fn get_page_address(&self, index: usize) -> Option<u32> {
        if !self.is_present(index) {
            None
        } else {
            Some(self.entries[index] & 0xFFFFF000)
        }
    }

    /// Get a pointer to this Page Table
    pub fn as_ptr(&self) -> *const PageTable {
        self as *const _
    }
}

impl Default for PageTable {
    fn default() -> Self {
        Self::new()
    }
}
