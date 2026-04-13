use super::cursor::{move_cursor, set_cursor, set_cursor_x, CURSOR};

// Assuming you have these constants defined
pub const VGA_BUFFER: *mut u16 = 0xB8000 as *mut u16;
pub const VGA_WIDTH: usize = 80;
pub const VGA_HEIGHT: usize = 25;

// Color constants
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ColorCode(pub u8);

impl ColorCode {
    pub fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[allow(dead_code)]
pub fn clear(color: ColorCode) {
    let blank = 0x20 | ((color.0 as u16) << 8);

    unsafe {
        for y in 0..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                let index = y * VGA_WIDTH + x;
                VGA_BUFFER.offset(index as isize).write_volatile(blank);
            }
        }
    }
    // Reset cursor to top-left after clearing
    set_cursor(0, 0);
}

#[allow(dead_code)]
pub fn print(str: &str, color: ColorCode) {
    for &byte in str.as_bytes().iter() {
        if byte == b'\n' {
            set_cursor_x(0);
            move_cursor(0, 1);
            // Scroll if we went past the bottom
            unsafe {
                if CURSOR.y >= VGA_HEIGHT as u16 {
                    scroll();
                }
            }
            continue;
        }
        unsafe {
            VGA_BUFFER
                .offset((CURSOR.y * (VGA_WIDTH as u16) + CURSOR.x) as isize)
                .write_volatile((byte as u16) | (color.0 as u16) << 8);
        }
        move_cursor(1, 0);
        // Wrap to next line if we reached the end of the row
        unsafe {
            if CURSOR.x >= VGA_WIDTH as u16 {
                set_cursor_x(0);
                move_cursor(0, 1);
            }
            // Scroll if we went past the bottom
            if CURSOR.y >= VGA_HEIGHT as u16 {
                scroll();
            }
        }
    }
}

#[allow(dead_code)]
pub fn print_char(c: char, color: ColorCode) {
    let byte = c as u8; // Convert the char to a byte
    let vga_char = (byte as u16) | (color.0 as u16) << 8;

    unsafe {
        VGA_BUFFER
            .offset((CURSOR.y * (VGA_WIDTH as u16) + CURSOR.x) as isize)
            .write_volatile(vga_char);
    }
    move_cursor(1, 0);
    // Wrap to next line if we reached the end of the row
    unsafe {
        if CURSOR.x >= VGA_WIDTH as u16 {
            set_cursor_x(0);
            move_cursor(0, 1);
        }
        // Scroll if we went past the bottom
        if CURSOR.y >= VGA_HEIGHT as u16 {
            scroll();
        }
    }
}

#[allow(dead_code)]
pub fn newline() {
    unsafe {
        let mut index = VGA_WIDTH * (VGA_HEIGHT - 1);
        while index < VGA_WIDTH * VGA_HEIGHT {
            VGA_BUFFER.offset(index as isize).write_volatile(0x20);
            index += 1;
        }
    }
}

#[allow(dead_code)]
pub fn scroll() {
    // Move each row up by one
    for y in 1..VGA_HEIGHT {
        for x in 0..VGA_WIDTH {
            unsafe {
                let src = y * VGA_WIDTH + x;
                let dst = (y - 1) * VGA_WIDTH + x;
                let vga_char = VGA_BUFFER.offset(src as isize).read_volatile();
                VGA_BUFFER.offset(dst as isize).write_volatile(vga_char);
            }
        }
    }
    // Clear the last line
    let blank = 0x20u16 | ((Color::White as u16) << 8); // space with default color
    for x in 0..VGA_WIDTH {
        unsafe {
            let index = (VGA_HEIGHT - 1) * VGA_WIDTH + x;
            VGA_BUFFER.offset(index as isize).write_volatile(blank);
        }
    }
    // Place cursor on the last line, preserving x position
    unsafe {
        set_cursor(CURSOR.x, (VGA_HEIGHT - 1) as u16);
    }
}

/// Erase the character behind the cursor (backspace)
#[allow(dead_code)]
pub fn backspace() {
    unsafe {
        // Don't backspace past the beginning of the screen
        if CURSOR.x == 0 && CURSOR.y == 0 {
            return;
        }

        // Move cursor back one position
        if CURSOR.x > 0 {
            move_cursor(-1, 0);
        } else {
            // Wrap to end of previous line
            set_cursor_x((VGA_WIDTH - 1) as u16);
            move_cursor(0, -1);
        }

        // Write a blank space at the cursor position
        let blank = 0x20u16 | ((Color::White as u16) << 8);
        VGA_BUFFER
            .offset((CURSOR.y * (VGA_WIDTH as u16) + CURSOR.x) as isize)
            .write_volatile(blank);
    }
}
