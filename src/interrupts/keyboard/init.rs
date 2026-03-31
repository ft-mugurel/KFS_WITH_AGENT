use crate::vga::text_mod::out::{print_char, backspace};
use crate::vga::text_mod::out::ColorCode;
use crate::vga::text_mod::out::Color;
use crate::interrupts::idt::register_interrupt_handler;
use crate::x86::io::outb;
use crate::interrupts::keyboard::caracter_map::*;
use crate::shell::SHELL;


#[no_mangle]
pub extern "C" fn keyboard_interrupt_handler() {
    // Read scancode from PS/2 keyboard data port (0x60)
    let scancode: u8 = unsafe {
        let mut code: u8;
        core::arch::asm!("in al, dx", out("al") code, in("dx") 0x60);
        code
    };
    // split the scancode to press and release
    let pressed = scancode & 0x80 == 0;
    let scancode = scancode & 0x7F;
    let _released = scancode & 0x80 != 0;

    // Only handle key presses (not releases)
    if pressed && scancode < 128 {
        // Handle Enter (scancode 0x1C)
        if scancode == 0x1C {
            unsafe {
                (*(&raw mut SHELL)).on_char('\n');
            }
        }
        // Handle Backspace (scancode 0x0E)
        else if scancode == 0x0E {
            unsafe {
                // Only erase on screen if the shell buffer has something to delete
                if SHELL.pos > 0 {
                    backspace();
                }
                (*(&raw mut SHELL)).on_char('\x08');
            }
        }
        // Handle printable characters
        else if let Some(ch) = LOWER_CARACTER_MAP[scancode as usize] {
            // Echo character to screen
            print_char(ch, ColorCode::new(Color::White, Color::Black));
            // Feed to shell
            unsafe {
                (*(&raw mut SHELL)).on_char(ch);
            }
        }
    }

    // Send End of Interrupt (EOI) to master PIC
    unsafe {
        outb(0x20, 0x20);
    }
}

extern "C" {
    fn isr_keyboard(); // the ISR we defined in NASM
}


pub fn init_keyboard() {
    unsafe {
        register_interrupt_handler(33, isr_keyboard); // IRQ1 = IDT index 32 + 1 = 33
    }
}

