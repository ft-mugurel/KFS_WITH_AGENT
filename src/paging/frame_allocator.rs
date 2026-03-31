//! Physical Frame Allocator — bitmap-based.
//!
//! Tracks which 4KB physical frames are free or used via a compact bit-array.
//! Each bit represents one 4KB frame:
//!   - 0 = free
//!   - 1 = used / allocated
//!
//! The allocator is initialized once during boot.  Everything below the
//! kernel image (BIOS area, VGA, kernel code, .bss, stacks) is permanently
//! marked as *used*.  Only the region from `_kernel_end` up to the total
//! physical memory is available for allocation.
//!
//! ## Memory budget
//! For 64 MB of RAM the bitmap is only 2 KB (512 × u32).

use super::mapper::PAGE_SIZE;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum physical memory we track (64 MB, matches our identity mapping).
pub const MAX_MEMORY: usize = 64 * 1024 * 1024;

/// Total number of 4 KB frames that fit in [`MAX_MEMORY`].
pub const MAX_FRAMES: usize = MAX_MEMORY / PAGE_SIZE; // 16 384

/// Bitmap width — each `u32` word tracks 32 consecutive frames.
const BITS_PER_WORD: usize = 32;

/// Number of `u32` words in the bitmap.
const BITMAP_SIZE: usize = MAX_FRAMES / BITS_PER_WORD; // 512

// ---------------------------------------------------------------------------
// Frame Allocator
// ---------------------------------------------------------------------------

/// Bitmap-based physical frame allocator.
pub struct FrameAllocator {
    /// One bit per 4 KB frame.  Index 0 = physical address 0x0000_0000.
    bitmap: [u32; BITMAP_SIZE],

    /// How many frames the machine actually has (≤ MAX_FRAMES).
    total_frames: usize,

    /// How many of those frames are currently marked *used*.
    used_frames: usize,

    /// Hint: word index where the last successful allocation happened.
    /// Next `alloc_frame` starts scanning here to avoid re-scanning the
    /// beginning of the bitmap every time.
    next_free_hint: usize,
}

impl FrameAllocator {
    // -- Construction -------------------------------------------------------

    /// Create a new allocator.
    ///
    /// All counters are zero and the bitmap is zeroed (goes into `.bss`).
    /// You **must** call [`init`](Self::init) before using any other method.
    pub const fn new() -> Self {
        FrameAllocator {
            bitmap: [0; BITMAP_SIZE],
            total_frames: 0,
            used_frames: 0,
            next_free_hint: 0,
        }
    }

    // -- Initialization -----------------------------------------------------

    /// Initialize the allocator for the given machine.
    ///
    /// 1. Marks **all** frames as *used* (safe default).
    /// 2. Frees the region from `kernel_end` (rounded up to the next page)
    ///    up to `total_memory`.
    ///
    /// Everything below `kernel_end` (first 1 MB BIOS/VGA area, kernel
    /// `.text`, `.rodata`, `.data`, `.bss` — which includes the page
    /// directory, page tables, and this bitmap — plus the kernel and user
    /// stacks) stays permanently marked as used.
    ///
    /// # Arguments
    /// * `total_memory` – total physical RAM in bytes (clamped to
    ///   [`MAX_MEMORY`]).
    /// * `kernel_end`   – first byte *past* the kernel image (the value of
    ///   the `_kernel_end` linker symbol).
    pub fn init(&mut self, total_memory: usize, kernel_end: usize) {
        // Clamp to the maximum we can track.
        let memory = if total_memory > MAX_MEMORY {
            MAX_MEMORY
        } else {
            total_memory
        };

        self.total_frames = memory / PAGE_SIZE;

        // Step 1 — mark every frame as *used*.
        for word in self.bitmap.iter_mut() {
            *word = 0xFFFF_FFFF;
        }
        self.used_frames = self.total_frames;

        // Step 2 — free everything from kernel_end to the top of RAM.
        let first_free_addr = (kernel_end + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
        let first_free_frame = first_free_addr / PAGE_SIZE;

        for idx in first_free_frame..self.total_frames {
            self.clear_bit(idx);
        }
        // Recount used frames exactly.
        self.used_frames = first_free_frame;

        // Start the hint right at the first free frame.
        self.next_free_hint = first_free_frame / BITS_PER_WORD;
    }

    // -- Allocation ---------------------------------------------------------

    /// Allocate one 4 KB frame.
    ///
    /// Scans the bitmap starting from an internal hint so repeated
    /// allocations are O(1) amortised rather than O(n) every time.
    ///
    /// Returns the **physical address** of the frame, or `None` if every
    /// frame is in use.
    pub fn alloc_frame(&mut self) -> Option<u32> {
        // First pass: scan from hint to end.
        if let Some(addr) = self.scan_and_alloc(self.next_free_hint, BITMAP_SIZE) {
            return Some(addr);
        }
        // Wrap-around pass: scan from 0 to hint.
        if let Some(addr) = self.scan_and_alloc(0, self.next_free_hint) {
            return Some(addr);
        }
        None // out of memory
    }

    /// Allocate one 4 KB frame **and zero its contents**.
    ///
    /// This is the right choice when allocating a page that will be used as
    /// a Page Table — a non-zeroed entry could look like a valid mapping.
    ///
    /// # Safety
    /// Requires that identity mapping is active (virtual addr = physical
    /// addr) so we can write to the returned physical address directly.
    pub unsafe fn alloc_frame_zeroed(&mut self) -> Option<u32> {
        let frame = self.alloc_frame()?;
        core::ptr::write_bytes(frame as *mut u8, 0, PAGE_SIZE);
        Some(frame)
    }

    // -- Deallocation -------------------------------------------------------

    /// Return a previously allocated frame to the free pool.
    ///
    /// Does nothing if the address is out of range or the frame is already
    /// free (double-free is silently ignored rather than panicking — safer
    /// for a kernel).
    pub fn free_frame(&mut self, phys_addr: u32) {
        let idx = phys_addr as usize / PAGE_SIZE;
        if idx >= self.total_frames {
            return;
        }
        let word = idx / BITS_PER_WORD;
        let bit = idx % BITS_PER_WORD;

        // Only decrement the counter if it was actually used.
        if self.bitmap[word] & (1 << bit) != 0 {
            self.bitmap[word] &= !(1u32 << bit);
            self.used_frames -= 1;

            // Move the hint backward so the next alloc will find this one.
            if word < self.next_free_hint {
                self.next_free_hint = word;
            }
        }
    }

    // -- Explicit mark helpers (for reserving known regions) ----------------

    /// Force-mark a frame as *used*.  No-op if already used.
    pub fn mark_used(&mut self, phys_addr: u32) {
        let idx = phys_addr as usize / PAGE_SIZE;
        if idx >= self.total_frames {
            return;
        }
        let word = idx / BITS_PER_WORD;
        let bit = idx % BITS_PER_WORD;
        if self.bitmap[word] & (1 << bit) == 0 {
            self.bitmap[word] |= 1u32 << bit;
            self.used_frames += 1;
        }
    }

    /// Force-mark a frame as *free*.  Same as [`free_frame`](Self::free_frame).
    pub fn mark_free(&mut self, phys_addr: u32) {
        self.free_frame(phys_addr);
    }

    // -- Queries ------------------------------------------------------------

    /// Is the frame at `phys_addr` currently marked as used?
    /// Out-of-range addresses are reported as used (conservative).
    pub fn is_used(&self, phys_addr: u32) -> bool {
        let idx = phys_addr as usize / PAGE_SIZE;
        if idx >= self.total_frames {
            return true;
        }
        let word = idx / BITS_PER_WORD;
        let bit = idx % BITS_PER_WORD;
        self.bitmap[word] & (1 << bit) != 0
    }

    /// Number of frames currently in use.
    pub fn used_count(&self) -> usize {
        self.used_frames
    }

    /// Number of frames currently free.
    pub fn free_count(&self) -> usize {
        self.total_frames.saturating_sub(self.used_frames)
    }

    /// Total number of tracked frames.
    pub fn total_count(&self) -> usize {
        self.total_frames
    }

    // -- Private helpers ----------------------------------------------------

    /// Scan bitmap words `[from .. to)` for the first free bit.
    /// If found, mark it used, update the hint, and return the physical
    /// address.
    fn scan_and_alloc(&mut self, from: usize, to: usize) -> Option<u32> {
        for word_idx in from..to {
            // Fast skip: if every bit in this word is 1, no free frames here.
            if self.bitmap[word_idx] == 0xFFFF_FFFF {
                continue;
            }

            let word = self.bitmap[word_idx];
            // Find the first 0-bit.
            for bit in 0..BITS_PER_WORD {
                if word & (1 << bit) == 0 {
                    let frame_idx = word_idx * BITS_PER_WORD + bit;
                    if frame_idx >= self.total_frames {
                        return None;
                    }

                    self.bitmap[word_idx] |= 1u32 << bit;
                    self.used_frames += 1;
                    self.next_free_hint = word_idx;

                    return Some((frame_idx * PAGE_SIZE) as u32);
                }
            }
        }
        None
    }

    /// Clear (free) one bit by frame index.  Does **not** update counters.
    fn clear_bit(&mut self, frame_idx: usize) {
        let word = frame_idx / BITS_PER_WORD;
        let bit = frame_idx % BITS_PER_WORD;
        self.bitmap[word] &= !(1u32 << bit);
    }
}

// ---------------------------------------------------------------------------
// Global instance
// ---------------------------------------------------------------------------

/// Single global frame-allocator (lives in `.bss` — zero cost at load time).
static mut FRAME_ALLOCATOR: FrameAllocator = FrameAllocator::new();

/// Obtain a mutable reference to the global frame allocator.
///
/// # Safety (internal)
/// The kernel is single-threaded and interrupts that touch the allocator
/// are not re-entrant, so the `&mut` is sound in practice.
pub fn get_frame_allocator() -> &'static mut FrameAllocator {
    unsafe { &mut *(&raw mut FRAME_ALLOCATOR) }
}
