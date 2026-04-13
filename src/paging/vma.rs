//! Virtual Memory Area (VMA) tracker.
//!
//! Every mapped region of virtual address space is described by a [`Vma`]
//! entry.  The kernel keeps a global, fixed-size table of these entries so
//! that:
//!
//! 1. The **page-fault handler** can decide whether a fault is recoverable
//!    (the address falls inside a known region whose pages are lazily
//!    allocated) or fatal (the address is truly invalid).
//! 2. Diagnostic commands (`vma`) can show the full virtual memory layout.
//! 3. Future subsystems (process isolation, mmap, etc.) have a single
//!    source of truth about what lives where.
//!
//! ## Design
//!
//! The table is a simple static array — no heap allocation needed, no
//! linked-list pointers.  With `MAX_VMAS = 32` and each entry being ~20
//! bytes, the whole table fits in < 1 KB of `.bss`.

use super::mapper::PAGE_SIZE;

// ---------------------------------------------------------------------------
// Region kind
// ---------------------------------------------------------------------------

/// What a VMA region is used for.
#[derive(Copy, Clone, PartialEq)]
pub enum VmaKind {
    /// Identity map (virtual == physical).  Always present.
    Identity,
    /// Kernel code / data / bss — loaded by the bootloader.
    KernelImage,
    /// Kernel stack (grows downward).
    KernelStack,
    /// User-mode stack (grows downward).
    UserStack,
    /// Guard page — intentionally unmapped to catch overflows.
    Guard,
    /// Kernel heap — pages allocated on demand.
    Heap,
    /// Memory-mapped I/O (e.g. VGA framebuffer at 0xB8000).
    Mmio,
}

// ---------------------------------------------------------------------------
// Permission flags
// ---------------------------------------------------------------------------

/// Bitflags describing what accesses are permitted.
#[derive(Copy, Clone)]
pub struct VmaFlags(pub u8);

impl VmaFlags {
    pub const NONE:     u8 = 0;
    pub const READ:     u8 = 1 << 0;
    pub const WRITE:    u8 = 1 << 1;
    pub const EXEC:     u8 = 1 << 2;
    pub const USER:     u8 = 1 << 3;
    /// Pages in this region are mapped on first access (demand paging).
    pub const DEMAND:   u8 = 1 << 4;

    pub const fn new(bits: u8) -> Self { Self(bits) }
    pub const fn contains(&self, bit: u8) -> bool { self.0 & bit != 0 }

    pub const fn readable(&self)  -> bool { self.contains(Self::READ) }
    pub const fn writable(&self)  -> bool { self.contains(Self::WRITE) }
    pub const fn user(&self)      -> bool { self.contains(Self::USER) }
    pub const fn demand(&self)    -> bool { self.contains(Self::DEMAND) }
}

// ---------------------------------------------------------------------------
// VMA entry
// ---------------------------------------------------------------------------

/// Describes one contiguous region of virtual address space.
#[derive(Copy, Clone)]
pub struct Vma {
    /// First byte of the region (page-aligned).
    pub start: u32,
    /// One-past-the-end (page-aligned).  The region is `[start, end)`.
    pub end: u32,
    /// What this region is for.
    pub kind: VmaKind,
    /// Permission / behaviour flags.
    pub flags: VmaFlags,
    /// Whether this slot is in use.
    pub active: bool,
}

impl Vma {
    pub const fn empty() -> Self {
        Vma {
            start: 0,
            end: 0,
            kind: VmaKind::Identity,
            flags: VmaFlags::new(VmaFlags::NONE),
            active: false,
        }
    }

    /// Size in bytes.
    pub const fn size(&self) -> u32 {
        self.end - self.start
    }

    /// Does this VMA contain the given virtual address?
    pub const fn contains(&self, addr: u32) -> bool {
        self.active && addr >= self.start && addr < self.end
    }

    /// Human-readable name for the kind.
    pub const fn kind_name(&self) -> &'static str {
        match self.kind {
            VmaKind::Identity    => "Identity",
            VmaKind::KernelImage => "KernelImg",
            VmaKind::KernelStack => "KernStack",
            VmaKind::UserStack   => "UserStack",
            VmaKind::Guard       => "Guard",
            VmaKind::Heap        => "Heap",
            VmaKind::Mmio        => "MMIO",
        }
    }

    /// Short flag string like "RW--" or "RWX-".
    pub fn flags_str(&self) -> [u8; 5] {
        let f = self.flags;
        [
            if f.readable()  { b'R' } else { b'-' },
            if f.writable()  { b'W' } else { b'-' },
            if f.contains(VmaFlags::EXEC)   { b'X' } else { b'-' },
            if f.user()      { b'U' } else { b'-' },
            if f.demand()    { b'D' } else { b'-' },
        ]
    }
}

// ---------------------------------------------------------------------------
// Global VMA table
// ---------------------------------------------------------------------------

/// Maximum number of tracked regions.
const MAX_VMAS: usize = 32;

/// The global VMA table (lives in `.bss`).
static mut VMA_TABLE: [Vma; MAX_VMAS] = [Vma::empty(); MAX_VMAS];

/// Number of active entries (cached for fast iteration).
static mut VMA_COUNT: usize = 0;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Register a new VMA region.  Returns `true` on success, `false` if the
/// table is full.
pub fn register(start: u32, end: u32, kind: VmaKind, flags: VmaFlags) -> bool {
    unsafe {
        let count = &mut *(&raw mut VMA_COUNT);
        if *count >= MAX_VMAS {
            return false;
        }
        let table = &mut *(&raw mut VMA_TABLE);
        for slot in table.iter_mut() {
            if !slot.active {
                slot.start  = start & 0xFFFFF000;
                slot.end    = (end + PAGE_SIZE as u32 - 1) & 0xFFFFF000_u32;
                slot.kind   = kind;
                slot.flags  = flags;
                slot.active = true;
                *count += 1;
                return true;
            }
        }
        false
    }
}

/// Remove a VMA by matching its start address.  Returns `true` if found.
pub fn unregister(start: u32) -> bool {
    unsafe {
        let table = &mut *(&raw mut VMA_TABLE);
        for slot in table.iter_mut() {
            if slot.active && slot.start == start {
                slot.active = false;
                let count = &mut *(&raw mut VMA_COUNT);
                *count -= 1;
                return true;
            }
        }
        false
    }
}

/// Look up the VMA that contains `addr`.  Returns `None` if the address
/// does not belong to any registered region.
pub fn find(addr: u32) -> Option<Vma> {
    unsafe {
        let table = &*(&raw const VMA_TABLE);
        for slot in table.iter() {
            if slot.contains(addr) {
                return Some(*slot);
            }
        }
        None
    }
}

/// Update the `end` field of the VMA whose `start` matches.
/// Used by the heap grower to extend the heap VMA in-place.
pub fn extend(start: u32, new_end: u32) -> bool {
    unsafe {
        let table = &mut *(&raw mut VMA_TABLE);
        for slot in table.iter_mut() {
            if slot.active && slot.start == start {
                slot.end = (new_end + PAGE_SIZE as u32 - 1) & 0xFFFFF000_u32;
                return true;
            }
        }
        false
    }
}

/// Return the number of active VMAs.
pub fn count() -> usize {
    unsafe { *(&raw const VMA_COUNT) }
}

/// Iterate over all active VMAs (in table order, not sorted by address).
///
/// The callback receives each active VMA.  This is the only safe way to
/// walk the table without exposing the mutable static directly.
pub fn for_each<F: FnMut(&Vma)>(mut f: F) {
    unsafe {
        let table = &*(&raw const VMA_TABLE);
        for slot in table.iter() {
            if slot.active {
                f(slot);
            }
        }
    }
}
