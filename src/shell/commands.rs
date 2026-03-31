use crate::vga::text_mod::out::{print, print_char, Color, ColorCode, clear};
use crate::x86::io::{inb, outb};
use crate::paging::mapper;
use crate::heap;

// Linker-provided stack symbols (declared in kernel.rs extern block)
extern "C" {
    pub static kernel_stack_top: u8;
    pub static kernel_stack_bottom: u8;
    pub static user_stack_top: u8;
    pub static user_stack_bottom: u8;
}

/// Print help text listing all commands
pub fn cmd_help() {
    print("Available commands:\n", ColorCode::new(Color::White, Color::Black));
    print("  help       - Show this help message\n", ColorCode::new(Color::White, Color::Black));
    print("  stack      - Print kernel stack info (base, top, size)\n", ColorCode::new(Color::White, Color::Black));
    print("  userstack  - Print user stack info (base, top, size)\n", ColorCode::new(Color::White, Color::Black));
    print("  reboot     - Reboot the system\n", ColorCode::new(Color::White, Color::Black));
    print("  halt       - Halt the CPU\n", ColorCode::new(Color::White, Color::Black));
    print("  clear      - Clear the screen\n", ColorCode::new(Color::White, Color::Black));
    print("  echo <msg> - Echo the given message\n", ColorCode::new(Color::White, Color::Black));
    print("  calc <expr>- Calculate: 1+1, 10/2, 5*3, 8-4\n", ColorCode::new(Color::White, Color::Black));
    print("  paging     - Show paging info and run live tests\n", ColorCode::new(Color::White, Color::Black));
    print("\nMath expressions (no space needed):\n", ColorCode::new(Color::LightCyan, Color::Black));
    print("  1+1        - Addition\n", ColorCode::new(Color::White, Color::Black));
    print("  10-3       - Subtraction\n", ColorCode::new(Color::White, Color::Black));
    print("  4*5        - Multiplication\n", ColorCode::new(Color::White, Color::Black));
    print("  20/4       - Division\n", ColorCode::new(Color::White, Color::Black));
    print("  5/0        - Division by zero (triggers CPU exception 0!)\n", ColorCode::new(Color::Yellow, Color::Black));
}

/// Print kernel stack information
pub fn cmd_stack() {
    unsafe {
        let top = &kernel_stack_top as *const u8 as usize;
        let bot = &kernel_stack_bottom as *const u8 as usize;
        let size = top - bot;

        print("Kernel Stack:\n", ColorCode::new(Color::Cyan, Color::Black));
        print("  Bottom: 0x", ColorCode::new(Color::White, Color::Black));
        print_hex(bot);
        print("\n", ColorCode::new(Color::White, Color::Black));

        print("  Top:    0x", ColorCode::new(Color::White, Color::Black));
        print_hex(top);
        print("\n", ColorCode::new(Color::White, Color::Black));

        print("  Size:   ", ColorCode::new(Color::White, Color::Black));
        print_dec(size);
        print(" bytes (", ColorCode::new(Color::White, Color::Black));
        print_dec(size / 1024);
        print(" KB)\n", ColorCode::new(Color::White, Color::Black));
    }
}

/// Print user stack information
pub fn cmd_userstack() {
    unsafe {
        let top = &user_stack_top as *const u8 as usize;
        let bot = &user_stack_bottom as *const u8 as usize;
        let size = top - bot;

        print("User Stack:\n", ColorCode::new(Color::Magenta, Color::Black));
        print("  Bottom: 0x", ColorCode::new(Color::White, Color::Black));
        print_hex(bot);
        print("\n", ColorCode::new(Color::White, Color::Black));

        print("  Top:    0x", ColorCode::new(Color::White, Color::Black));
        print_hex(top);
        print("\n", ColorCode::new(Color::White, Color::Black));

        print("  Size:   ", ColorCode::new(Color::White, Color::Black));
        print_dec(size);
        print(" bytes (", ColorCode::new(Color::White, Color::Black));
        print_dec(size / 1024);
        print(" KB)\n", ColorCode::new(Color::White, Color::Black));
    }
}

/// Reboot via keyboard controller reset (0x64 port, 0xFE command)
pub fn cmd_reboot() {
    print("Rebooting...\n", ColorCode::new(Color::Yellow, Color::Black));
    unsafe {
        // Wait for keyboard controller input buffer to be empty
        loop {
            let status = inb(0x64);
            if status & 0x02 == 0 {
                break;
            }
        }
        // Send CPU reset command to keyboard controller
        outb(0x64, 0xFE);
    }
    // If that didn't work, loop forever (CPU will be reset eventually)
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Halt the CPU in a loop
pub fn cmd_halt() {
    print("Halting CPU. Press reset to continue.\n", ColorCode::new(Color::Yellow, Color::Black));
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

/// Clear the screen
pub fn cmd_clear() {
    clear(ColorCode::new(Color::White, Color::Black));
}

/// Echo the given arguments
pub fn cmd_echo(args: &str) {
    if args.is_empty() {
        print("\n", ColorCode::new(Color::White, Color::Black));
    } else {
        print(args, ColorCode::new(Color::White, Color::Black));
        print("\n", ColorCode::new(Color::White, Color::Black));
    }
}

/// Calculate a simple math expression like "1+1", "10/2", "5*3", "8-4"
/// Supports: +, -, *, /
/// Division by zero will trigger CPU exception 0 (Division Error)
pub fn cmd_calc(expr: &str) {
    let expr = expr.trim();
    
    if expr.is_empty() {
        print("Usage: <number><op><number>  e.g. 1+1, 10/2, 5*3\n", ColorCode::new(Color::LightCyan, Color::Black));
        print("Supports: +  -  *  /\n", ColorCode::new(Color::LightCyan, Color::Black));
        print("Try '5/0' to trigger a division by zero error!\n", ColorCode::new(Color::Yellow, Color::Black));
        return;
    }

    // Try to parse as a math expression
    if let Some(result) = parse_and_eval(expr) {
        print(expr, ColorCode::new(Color::LightCyan, Color::Black));
        print(" = ", ColorCode::new(Color::White, Color::Black));
        print_dec(result);
        print("\n", ColorCode::new(Color::White, Color::Black));
    } else {
        print("Invalid expression: ", ColorCode::new(Color::LightRed, Color::Black));
        print(expr, ColorCode::new(Color::LightRed, Color::Black));
        print("\nExpected format: <number><op><number>\n", ColorCode::new(Color::White, Color::Black));
        print("Example: 10+5, 20/4, 7*3, 100-50\n", ColorCode::new(Color::White, Color::Black));
    }
}

/// Try to parse and evaluate a simple binary expression like "10+5"
/// Returns Some(result) on success, None on parse failure
/// Division by zero will panic (trigger CPU exception 0)
fn parse_and_eval(expr: &str) -> Option<usize> {
    let bytes = expr.as_bytes();
    
    // Find the operator (look for first +, -, *, / that's not at the start)
    let mut op_idx = None;
    let mut op_char = b' ';
    
    for (i, &b) in bytes.iter().enumerate() {
        if i == 0 {
            continue; // Skip if operator is first char (negative numbers not supported)
        }
        if b == b'+' || b == b'-' || b == b'*' || b == b'/' {
            op_idx = Some(i);
            op_char = b;
            break;
        }
    }
    
    let op_idx = op_idx?;
    
    // Parse left operand
    let left_str = core::str::from_utf8(&bytes[..op_idx]).ok()?;
    let left: usize = left_str.parse().ok()?;
    
    // Parse right operand
    let right_str = core::str::from_utf8(&bytes[op_idx + 1..]).ok()?;
    let right: usize = right_str.parse().ok()?;
    
    // Perform the operation
    let result = match op_char {
        b'+' => left + right,
        b'-' => left.saturating_sub(right), // Avoid underflow
        b'*' => left * right,
        b'/' => {
            if right == 0 {
                // This will trigger CPU exception 0 (Division Error)
                left / right
            } else {
                left / right
            }
        }
        _ => return None,
    };
    
    Some(result)
}

/// Display full paging subsystem status and run a live mapping test.
pub fn cmd_paging() {
    let cyan  = ColorCode::new(Color::LightCyan, Color::Black);
    let white = ColorCode::new(Color::White, Color::Black);
    let green = ColorCode::new(Color::LightGreen, Color::Black);
    let yel   = ColorCode::new(Color::Yellow, Color::Black);

    // ── 1. Frame Allocator ─────────────────────────────────────────────
    print("=== Frame Allocator ===\n", cyan);
    let fa = crate::paging::get_frame_allocator();
    let total  = fa.total_count();
    let used   = fa.used_count();
    let free   = fa.free_count();

    print("  Total : ", white);
    print_dec(total);
    print(" frames (", white);
    print_dec(total * 4096 / 1024);
    print(" KB)\n", white);

    print("  Used  : ", white);
    print_dec(used);
    print(" frames (", white);
    print_dec(used * 4096 / 1024);
    print(" KB)\n", white);

    print("  Free  : ", white);
    print_dec(free);
    print(" frames (", white);
    print_dec(free * 4096 / 1024);
    print(" KB)\n", white);

    // ── 2. Page Directory overview ─────────────────────────────────────
    print("\n=== Page Directory ===\n", cyan);
    let pd = mapper::get_page_directory();
    let mut present_count: usize = 0;
    for i in 0..1024 {
        if pd.is_present(i) {
            present_count += 1;
        }
    }
    print("  Present entries: ", white);
    print_dec(present_count);
    print(" / 1024\n", white);

    // Show identity-map range
    print("  [0-15]  Identity map  0x00000000-0x03FFFFFF\n", white);

    // Show any extra PDEs (heap, etc.)
    for i in 16..1024 {
        if pd.is_present(i) {
            print("  [", white);
            print_dec(i);
            print("]     Mapped        ", white);
            print_hex(i * 4 * 1024 * 1024);  // start virt addr
            print("-", white);
            print_hex((i + 1) * 4 * 1024 * 1024 - 1);
            print("\n", white);
        }
    }

    // ── 3. Kernel Heap ─────────────────────────────────────────────────
    print("\n=== Kernel Heap ===\n", cyan);
    print("  Region: ", white);
    print_hex(heap::HEAP_START);
    print(" - ", white);
    print_hex(heap::HEAP_START + heap::HEAP_INIT_SIZE - 1);
    print(" (", white);
    print_dec(heap::HEAP_INIT_SIZE / 1024);
    print(" KB)\n", white);

    print("  Used  : ", white);
    print_dec(heap::used());
    print(" bytes\n", white);

    print("  Free  : ", white);
    print_dec(heap::free());
    print(" bytes\n", white);

    // ── 4. Live mapping test ───────────────────────────────────────────
    print("\n=== Live Paging Test ===\n", cyan);

    // Use a virtual address way outside identity map (0x0800_0000 = 128MB).
    let test_virt: u32 = 0x0800_0000;

    // 4a. Allocate a physical frame
    print("  Allocating frame... ", white);
    let frame = crate::paging::get_frame_allocator()
        .alloc_frame();
    let frame_addr = match frame {
        Some(addr) => {
            print("OK (", green);
            print_hex(addr as usize);
            print(")\n", green);
            addr
        }
        None => {
            print("FAIL (out of memory)\n", ColorCode::new(Color::Red, Color::Black));
            return;
        }
    };

    // 4b. Map virtual -> physical
    print("  Mapping ", white);
    print_hex(test_virt as usize);
    print(" -> ", white);
    print_hex(frame_addr as usize);
    print(" ... ", white);
    unsafe { crate::paging::map_page(test_virt, frame_addr, true, false); }
    print("OK\n", green);

    // 4c. Write through the virtual address
    let magic: u32 = 0xDEAD_BEEF;
    print("  Writing 0xDEADBEEF... ", white);
    unsafe { core::ptr::write_volatile(test_virt as *mut u32, magic); }
    print("OK\n", green);

    // 4d. Read back through the virtual address
    print("  Reading back... ", white);
    let readback = unsafe { core::ptr::read_volatile(test_virt as *const u32) };
    print_hex(readback as usize);
    if readback == magic {
        print(" OK!\n", green);
    } else {
        print(" MISMATCH!\n", ColorCode::new(Color::Red, Color::Black));
    }

    // 4e. Translate via software
    print("  Translate ", white);
    print_hex(test_virt as usize);
    print(" -> ", white);
    match mapper::get_physical_address(test_virt) {
        Some(phys) => {
            print_hex(phys as usize);
            print(" OK\n", green);
        }
        None => {
            print("NONE (bug!)\n", ColorCode::new(Color::Red, Color::Black));
        }
    }

    // 4f. Unmap & free
    print("  Unmapping... ", white);
    unsafe { crate::paging::unmap_page(test_virt); }
    print("OK\n", green);

    print("  Freeing frame... ", white);
    crate::paging::get_frame_allocator().free_frame(frame_addr);
    print("OK\n", green);

    // ── 5. Heap allocation test (proves alloc crate works) ─────────────
    print("\n=== Heap Alloc Test ===\n", cyan);
    print("  Heap used before: ", white);
    print_dec(heap::used());
    print(" bytes\n", white);

    print("  Box::new(42u32)... ", white);
    let boxed = alloc::boxed::Box::new(42u32);
    print("OK (value = ", green);
    print_dec(*boxed as usize);
    print(")\n", green);

    print("  Vec push 1,2,3... ", white);
    let mut v = alloc::vec::Vec::new();
    v.push(1u32);
    v.push(2u32);
    v.push(3u32);
    print("OK (len = ", green);
    print_dec(v.len());
    print(")\n", green);

    print("  Heap used after : ", white);
    print_dec(heap::used());
    print(" bytes\n", white);

    print("\n", white);
    print("All paging tests passed!\n", yel);
}

// --- Helper functions for formatting ---

fn print_hex(val: usize) {
    print("0x", ColorCode::new(Color::LightCyan, Color::Black));
    // Simple hex print for u32-ish values
    let hex_chars = b"0123456789ABCDEF";
    // Print 8 hex digits (32-bit)
    for i in (0..8).rev() {
        let nibble = (val >> (i * 4)) & 0xF;
        let c = hex_chars[nibble] as char;
        print_char(c, ColorCode::new(Color::LightCyan, Color::Black));
    }
}

fn print_dec(val: usize) {
    if val == 0 {
        print("0", ColorCode::new(Color::White, Color::Black));
        return;
    }
    // Collect digits
    let mut digits: [u8; 20] = [0u8; 20];
    let mut n = val;
    let mut i = 0usize;
    while n > 0 {
        digits[i] = b'0' + (n % 10) as u8;
        n /= 10;
        i += 1;
    }
    // Print in reverse
    while i > 0 {
        i -= 1;
        let c = digits[i] as char;
        print_char(c, ColorCode::new(Color::White, Color::Black));
    }
}
