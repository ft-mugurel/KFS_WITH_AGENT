//! Page Directory implementation for x86 32-bit paging.
//!
//! The Page Directory is a 4KB page containing 1024 Page Directory Entries (PDEs).
//! Each PDE points to a Page Table (which covers 4MB of virtual address space).
//!
//! Layout:
//! - 1024 PDEs × 4 bytes = 4096 bytes (4KB)
//! - Each PDE covers 4MB of virtual address space (1024 × 4KB pages)
//! - Total coverage: 1024 × 4MB = 4GB

/// Flags for Page Directory Entries
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
    /// Page has been accessed (set by CPU)
    pub const ACCESSED: u32 = 1 << 5;
    /// Page size: 0 = 4KB pages, 1 = 4MB pages (if CR4.PSE=1)
    pub const PAGE_SIZE: u32 = 1 << 7;
}

/// A Page Directory Entry (PDE) is a 32-bit value containing:
/// - Bits 31:12 = Physical address of the Page Table (4KB aligned)
/// - Bits 11:0  = Flags (present, writable, user, etc.)
pub type PageDirectoryEntry = u32;

/// The Page Directory - must be 4KB aligned for CR3.
/// Contains 1024 PDEs, each covering 4MB of virtual address space.
#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct PageDirectory {
    /// Array of 1024 PDEs
    pub entries: [PageDirectoryEntry; 1024],
}

impl PageDirectory {
    /// Create a new empty Page Directory (all entries = 0 = not present)
    pub const fn new() -> Self {
        PageDirectory {
            entries: [0; 1024],
        }
    }

    /// Set a Page Directory Entry.
    ///
    /// # Arguments
    /// * `index` - PDE index (0-1023), corresponds to virtual address index * 4MB
    /// * `pt_physical_addr` - Physical address of the Page Table (must be 4KB aligned)
    /// * `flags` - Combination of flags:: constants
    ///
    /// # Panics
    /// Panics if pt_physical_addr is not 4KB aligned.
    pub fn set_entry(&mut self, index: usize, pt_physical_addr: u32, flags: u32) {
        debug_assert!(pt_physical_addr & 0xFFF == 0, "Page Table address must be 4KB aligned");
        debug_assert!(index < 1024, "PDE index out of range");
        
        // PDE format: upper 20 bits = address, lower 12 bits = flags
        self.entries[index] = (pt_physical_addr & 0xFFFFF000) | (flags & 0xFFF);
    }

    /// Clear a PDE (mark as not present)
    pub fn clear_entry(&mut self, index: usize) {
        debug_assert!(index < 1024, "PDE index out of range");
        self.entries[index] = 0;
    }

    /// Get the raw PDE value at the given index
    pub fn get_entry(&self, index: usize) -> PageDirectoryEntry {
        debug_assert!(index < 1024, "PDE index out of range");
        self.entries[index]
    }

    /// Check if a PDE is present
    pub fn is_present(&self, index: usize) -> bool {
        (self.entries[index] & flags::PRESENT) != 0
    }

    /// Get the physical address of the Page Table this PDE points to
    /// Returns None if not present
    pub fn get_pt_address(&self, index: usize) -> Option<u32> {
        if !self.is_present(index) {
            None
        } else {
            Some(self.entries[index] & 0xFFFFF000)
        }
    }

    /// Get a pointer to this Page Directory (for loading into CR3)
    pub fn as_ptr(&self) -> *const PageDirectory {
        self as *const _
    }

    /// Get the physical address of this Page Directory (for CR3)
    /// Note: Before paging is enabled, virtual = physical, so this works.
    pub fn physical_address(&self) -> u32 {
        self.as_ptr() as u32
    }
}

impl Default for PageDirectory {
    fn default() -> Self {
        Self::new()
    }
}
