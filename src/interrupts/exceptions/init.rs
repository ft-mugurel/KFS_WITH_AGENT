use crate::interrupts::idt::register_interrupt_handler;
use crate::vga::text_mod::out::{print, print_char, Color, ColorCode};

/// Print a u32 as 8 hex digits (e.g., "0xDEADBEEF").
/// Includes the "0x" prefix.
fn print_hex(val: u32) {
    let cyan = ColorCode::new(Color::LightCyan, Color::Black);
    print("0x", cyan);
    let hex_chars = b"0123456789ABCDEF";
    for i in (0..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        print_char(hex_chars[nibble] as char, cyan);
    }
}

// ============================================================================
// CPU Exception Handlers
// ============================================================================
// These are called from the assembly ISRs when a CPU exception occurs.
// They print diagnostic information and then halt for fatal exceptions.

#[no_mangle]
pub extern "C" fn exception_division_error() {
    print("\n[EXCEPTION] Division Error (0)\n", ColorCode::new(Color::Red, Color::Black));
    print("The CPU attempted to divide by zero or the divisor was too small.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_debug() {
    print("\n[EXCEPTION] Debug (1)\n", ColorCode::new(Color::Yellow, Color::Black));
    print("Debug exception triggered.\n", ColorCode::new(Color::White, Color::Black));
    // Don't halt - debug exceptions are often used for breakpoints
}

#[no_mangle]
pub extern "C" fn exception_breakpoint() {
    print("\n[EXCEPTION] Breakpoint (3)\n", ColorCode::new(Color::Yellow, Color::Black));
    print("INT3 breakpoint triggered.\n", ColorCode::new(Color::White, Color::Black));
    // Don't halt - breakpoints are used for debugging
}

#[no_mangle]
pub extern "C" fn exception_overflow() {
    print("\n[EXCEPTION] Overflow (4)\n", ColorCode::new(Color::Red, Color::Black));
    print("Overflow exception - result too large.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_bound_range() {
    print("\n[EXCEPTION] Bound Range Exceeded (5)\n", ColorCode::new(Color::Red, Color::Black));
    print("BOUND instruction detected out-of-range value.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_invalid_opcode() {
    print("\n[EXCEPTION] Invalid Opcode (6)\n", ColorCode::new(Color::Red, Color::Black));
    print("The CPU encountered an invalid or undefined instruction.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_device_not_available() {
    print("\n[EXCEPTION] Device Not Available (7)\n", ColorCode::new(Color::Red, Color::Black));
    print("FPU/MMX/SSE instruction used but device not available.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_double_fault() {
    print("\n[EXCEPTION] Double Fault (8)\n", ColorCode::new(Color::Red, Color::Black));
    print("A second exception occurred while handling the first!\n", ColorCode::new(Color::White, Color::Black));
    print("This is a fatal error. System halted.\n", ColorCode::new(Color::Red, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_coprocessor_segment() {
    print("\n[EXCEPTION] Coprocessor Segment Overrun (9)\n", ColorCode::new(Color::Red, Color::Black));
    print("Legacy FPU segment overrun.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_invalid_tss() {
    print("\n[EXCEPTION] Invalid TSS (10)\n", ColorCode::new(Color::Red, Color::Black));
    print("Task State Segment is invalid.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_segment_not_present() {
    print("\n[EXCEPTION] Segment Not Present (11)\n", ColorCode::new(Color::Red, Color::Black));
    print("A segment selector references a descriptor not present.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_stack_segment_fault() {
    print("\n[EXCEPTION] Stack Segment Fault (12)\n", ColorCode::new(Color::Red, Color::Black));
    print("Stack segment fault occurred.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_general_protection_fault() {
    print("\n[EXCEPTION] General Protection Fault (13)\n", ColorCode::new(Color::Red, Color::Black));
    print("General protection violation. Possible causes:\n", ColorCode::new(Color::White, Color::Black));
    print("  - Segment limit exceeded\n", ColorCode::new(Color::White, Color::Black));
    print("  - Privilege level violation\n", ColorCode::new(Color::White, Color::Black));
    print("  - Invalid segment selector\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

/// Page fault handler -- receives the CPU error code from the assembly ISR.
///
/// ## Smart fault resolution (Phase 3)
///
/// Before printing a fatal diagnostic the handler consults the VMA table:
///
/// | VMA kind | Action |
/// |----------|--------|
/// | `Guard`  | Print **STACK OVERFLOW** and halt. |
/// | `Heap` with DEMAND flag, page not present | **Demand-page**: allocate a physical frame, map it, and resume. |
/// | Anything else / no VMA | Fatal -- print full diagnostic and halt. |
///
/// Error code bits (set by the CPU):
///   Bit 0  P    -- 0 = page not present,  1 = protection violation
///   Bit 1  W/R  -- 0 = read access,       1 = write access
///   Bit 2  U/S  -- 0 = supervisor mode,   1 = user mode
///   Bit 3  RSVD -- 1 = reserved bit set in a page-table entry
///   Bit 4  I/D  -- 1 = fault caused by an instruction fetch
#[no_mangle]
pub extern "C" fn exception_page_fault(error_code: u32) {
    let red   = ColorCode::new(Color::Red, Color::Black);
    let white = ColorCode::new(Color::White, Color::Black);
    let yel   = ColorCode::new(Color::Yellow, Color::Black);
    let green = ColorCode::new(Color::LightGreen, Color::Black);

    // -- CR2: the virtual address that caused the fault --
    let fault_addr: u32;
    unsafe {
        core::arch::asm!("mov {0}, cr2", out(reg) fault_addr,
                         options(nomem, nostack, preserves_flags));
    }

    let present  = error_code & (1 << 0) != 0;

    // -- Consult the VMA table for smart resolution --
    use crate::paging::vma;
    use crate::paging::vma::VmaKind;

    if let Some(region) = vma::find(fault_addr) {
        // ---- Guard page hit = stack overflow ----
        if region.kind == VmaKind::Guard {
            print("\n[STACK OVERFLOW] ", red);
            print("at ", white);
            print_hex(fault_addr);
            print("\n", white);
            print("  The stack grew past its guard page!\n", white);
            print("  Guard region: ", white);
            print_hex(region.start);
            print(" - ", white);
            print_hex(region.end);
            print("\n", white);
            halt();
        }

        // ---- Demand paging for heap / demand-flagged regions ----
        if region.flags.demand() && !present {
            // The address is inside a known region that supports lazy
            // allocation, and the page is simply not present yet.
            // Allocate a frame, map it, zero it, and *return* so the
            // faulting instruction is re-executed.
            unsafe {
                let fa = crate::paging::get_frame_allocator();
                if let Some(frame) = fa.alloc_frame() {
                    let page_addr = fault_addr & 0xFFFFF000;
                    crate::paging::map_page(
                        page_addr,
                        frame,
                        region.flags.writable(),
                        region.flags.user(),
                    );
                    // Zero the page so no stale data leaks.
                    core::ptr::write_bytes(page_addr as *mut u8, 0, 4096);
                    return; // Resume -- the CPU will re-execute the instruction.
                }
            }
            // If we get here, alloc_frame returned None -- out of memory.
            print("\n[PAGE FAULT] Demand paging FAILED (out of frames) at ", red);
            print_hex(fault_addr);
            print("\n", white);
            halt();
        }
    }

    // -- No VMA match, or not a recoverable fault -- print full diagnostic --

    print("\n[PAGE FAULT] ", red);
    print("at ", white);
    print_hex(fault_addr);
    print("\n", white);

    // Error code
    print("  Error code: ", white);
    print_hex(error_code);
    print("\n", white);

    let write    = error_code & (1 << 1) != 0;
    let user     = error_code & (1 << 2) != 0;
    let reserved = error_code & (1 << 3) != 0;
    let fetch    = error_code & (1 << 4) != 0;

    // Present bit
    print("  P (present) : ", white);
    if present {
        print("1 - protection violation (page IS mapped)\n", yel);
    } else {
        print("0 - page not present (not mapped)\n", yel);
    }

    // Write/Read bit
    print("  W (write)   : ", white);
    if write {
        print("1 - caused by a WRITE\n", yel);
    } else {
        print("0 - caused by a READ\n", yel);
    }

    // User/Supervisor bit
    print("  U (user)    : ", white);
    if user {
        print("1 - user mode\n", yel);
    } else {
        print("0 - supervisor (kernel) mode\n", yel);
    }

    // Reserved-bit violation
    print("  R (reserved): ", white);
    if reserved {
        print("1 - reserved bit set in PTE!\n", red);
    } else {
        print("0 - no reserved-bit violation\n", green);
    }

    // Instruction fetch
    print("  I (fetch)   : ", white);
    if fetch {
        print("1 - instruction fetch\n", yel);
    } else {
        print("0 - data access\n", green);
    }

    // VMA context
    print("\n  VMA region: ", white);
    match vma::find(fault_addr) {
        Some(v) => {
            print(v.kind_name(), yel);
            print(" [", white);
            print_hex(v.start);
            print("-", white);
            print_hex(v.end);
            print("]\n", white);
        }
        None => {
            print("NONE (address not in any known region)\n", red);
        }
    }

    // One-line summary
    print("\n  => ", white);
    if user { print("User ", yel); } else { print("Kernel ", yel); }
    if write { print("WRITE", yel); } else if fetch { print("FETCH", yel); } else { print("READ", yel); }
    print(" to a ", white);
    if present { print("PRESENT", yel); } else { print("NON-PRESENT", yel); }
    print(" page at ", white);
    print_hex(fault_addr);
    print("\n", white);

    halt();
}

#[no_mangle]
pub extern "C" fn exception_x87_floating_point() {
    print("\n[EXCEPTION] x87 Floating-Point Exception (16)\n", ColorCode::new(Color::Red, Color::Black));
    print("x87 FPU floating-point error.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_alignment_check() {
    print("\n[EXCEPTION] Alignment Check (17)\n", ColorCode::new(Color::Red, Color::Black));
    print("Alignment check exception - misaligned memory access.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_machine_check() {
    print("\n[EXCEPTION] Machine Check (18)\n", ColorCode::new(Color::Red, Color::Black));
    print("Machine check exception - hardware error detected.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_simd_floating_point() {
    print("\n[EXCEPTION] SIMD Floating-Point Exception (19)\n", ColorCode::new(Color::Red, Color::Black));
    print("SSE/AVX floating-point error.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_control_protection() {
    print("\n[EXCEPTION] Control Protection Exception (21)\n", ColorCode::new(Color::Red, Color::Black));
    print("Control-flow protection violation.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_hypervisor_injection() {
    print("\n[EXCEPTION] Hypervisor Injection Exception (28)\n", ColorCode::new(Color::Red, Color::Black));
    print("Hypervisor injection exception.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_vmm_communication() {
    print("\n[EXCEPTION] VMM Communication Exception (29)\n", ColorCode::new(Color::Red, Color::Black));
    print("VMM communication exception.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

#[no_mangle]
pub extern "C" fn exception_security_exception() {
    print("\n[EXCEPTION] Security Exception (30)\n", ColorCode::new(Color::Red, Color::Black));
    print("Security exception - security violation.\n", ColorCode::new(Color::White, Color::Black));
    halt();
}

// ============================================================================
// Helper Functions
// ============================================================================

fn halt() -> ! {
    print("\nSystem halted.\n", ColorCode::new(Color::Red, Color::Black));
    loop {
        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

// ============================================================================
// Exception Initialization
// ============================================================================

extern "C" {
    fn isr_division_error();
    fn isr_debug();
    fn isr_breakpoint();
    fn isr_overflow();
    fn isr_bound_range();
    fn isr_invalid_opcode();
    fn isr_device_not_available();
    fn isr_double_fault();
    fn isr_coprocessor_segment();
    fn isr_invalid_tss();
    fn isr_segment_not_present();
    fn isr_stack_segment_fault();
    fn isr_general_protection_fault();
    fn isr_page_fault();
    fn isr_x87_floating_point();
    fn isr_alignment_check();
    fn isr_machine_check();
    fn isr_simd_floating_point();
    fn isr_control_protection();
    fn isr_hypervisor_injection();
    fn isr_vmm_communication();
    fn isr_security_exception();
}

pub fn init_exceptions() {
    unsafe {
        // Exceptions without error codes
        register_interrupt_handler(0, isr_division_error);
        register_interrupt_handler(1, isr_debug);
        register_interrupt_handler(3, isr_breakpoint);
        register_interrupt_handler(4, isr_overflow);
        register_interrupt_handler(5, isr_bound_range);
        register_interrupt_handler(6, isr_invalid_opcode);
        register_interrupt_handler(7, isr_device_not_available);
        register_interrupt_handler(9, isr_coprocessor_segment);
        register_interrupt_handler(16, isr_x87_floating_point);
        register_interrupt_handler(18, isr_machine_check);
        register_interrupt_handler(19, isr_simd_floating_point);
        register_interrupt_handler(28, isr_hypervisor_injection);

        // Exceptions with error codes (CPU pushes error code)
        register_interrupt_handler(8, isr_double_fault);
        register_interrupt_handler(10, isr_invalid_tss);
        register_interrupt_handler(11, isr_segment_not_present);
        register_interrupt_handler(12, isr_stack_segment_fault);
        register_interrupt_handler(13, isr_general_protection_fault);
        register_interrupt_handler(14, isr_page_fault);
        register_interrupt_handler(17, isr_alignment_check);
        register_interrupt_handler(21, isr_control_protection);
        register_interrupt_handler(29, isr_vmm_communication);
        register_interrupt_handler(30, isr_security_exception);
    }
}
