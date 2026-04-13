#![no_std]
#![no_main]

extern crate alloc;

pub mod x86;
pub mod interrupts;
pub mod vga;
pub mod gdt;
pub mod shell;
pub mod paging;
pub mod heap;

use core::panic::PanicInfo;

use gdt::gdt::load_gdt;

// Linker-provided stack symbols (defined in linker.ld)
extern "C" {
    pub static kernel_stack_top: u8;
    pub static kernel_stack_bottom: u8;
    pub static user_stack_top: u8;
    pub static user_stack_bottom: u8;
}

use interrupts::keyboard::init::init_keyboard;
use interrupts::idt::init_idt;
use interrupts::pic::init_pic;
use interrupts::utils::enable_interrupts;
use interrupts::exceptions::init::init_exceptions;
use paging::init as init_paging;
use vga::text_mod::out::{print, Color, ColorCode};
use shell::SHELL;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn kmain() -> ! {
    // Initialize paging FIRST - this enables virtual memory
    // Must be done before any other initialization that accesses memory
    // because after paging is enabled, unmapped addresses will fault
    unsafe {
        init_paging();
    }

    // Initialize the kernel heap (maps virtual pages above 64MB for alloc)
    unsafe {
        heap::init();
    }

    load_gdt();
    init_idt();
    unsafe {init_pic()};
    init_exceptions();
    init_keyboard();
    enable_interrupts();

    // Print welcome banner
    print("\n=== MTU Kernel Shell ===\n", ColorCode::new(Color::LightCyan, Color::Black));
    print("Paging: ENABLED (identity mapped 0-64MB)\n", ColorCode::new(Color::LightGreen, Color::Black));
    print("Type 'help' for available commands.\n\n", ColorCode::new(Color::White, Color::Black));

    // Show initial prompt
    unsafe {
        (*(&raw const SHELL)).prompt();
    }

    loop {}
}

