pub mod commands;

use crate::vga::text_mod::out::{print, Color, ColorCode};

// Linker-provided stack symbols (declared in kernel.rs extern block)
extern "C" {
    pub static kernel_stack_top: u8;
    pub static kernel_stack_bottom: u8;
    pub static user_stack_top: u8;
    pub static user_stack_bottom: u8;
}

const MAX_LINE_LEN: usize = 80;

pub struct Shell {
    buffer: [u8; MAX_LINE_LEN],
    pub pos: usize,
}

impl Shell {
    pub const fn new() -> Self {
        Shell {
            buffer: [0u8; MAX_LINE_LEN],
            pos: 0,
        }
    }

    pub fn prompt(&self) {
        print("> ", ColorCode::new(Color::LightGreen, Color::Black));
    }

    pub fn on_char(&mut self, c: char) {
        if c == '\n' || c == '\r' {
            // Enter pressed - process line
            print("\n", ColorCode::new(Color::White, Color::Black));
            self.process_line();
            self.pos = 0;
            self.prompt();
            return;
        }

        if c == '\x08' || c == '\x7f' {
            // Backspace - update internal buffer only (visual handled by keyboard handler)
            if self.pos > 0 {
                self.pos -= 1;
                self.buffer[self.pos] = 0;
            }
            return;
        }

        if self.pos < MAX_LINE_LEN - 1 {
            let b = c as u8;
            self.buffer[self.pos] = b;
            self.pos += 1;
            // Echo the character (already handled by keyboard, but just in case)
        }
    }

    fn process_line(&mut self) {
        if self.pos == 0 {
            return;
        }
        // Null-terminate for safety
        self.buffer[self.pos] = 0;

        // Convert to str
        let line = core::str::from_utf8(&self.buffer[..self.pos]).unwrap_or("");
        let line = line.trim();

        if line.is_empty() {
            return;
        }

        // Parse command and args into owned local buffers to avoid borrow conflicts
        let mut cmd_buf = [0u8; 16];
        let mut args_buf = [0u8; 64];
        let mut cmd_len = 0usize;
        let mut args_len = 0usize;

        let mut in_args = false;
        for b in line.as_bytes().iter() {
            if *b == b' ' && !in_args {
                in_args = true;
                continue;
            }
            if in_args {
                if args_len < args_buf.len() {
                    args_buf[args_len] = *b;
                    args_len += 1;
                }
            } else {
                if cmd_len < cmd_buf.len() {
                    cmd_buf[cmd_len] = *b;
                    cmd_len += 1;
                }
            }
        }

        // Convert to str
        let cmd = core::str::from_utf8(&cmd_buf[..cmd_len]).unwrap_or("");
        let args = core::str::from_utf8(&args_buf[..args_len]).unwrap_or("");

        self.execute(cmd, args);
    }

    fn execute(&mut self, cmd: &str, args: &str) {
        match cmd {
            "help" => commands::cmd_help(),
            "stack" => commands::cmd_stack(),
            "userstack" => commands::cmd_userstack(),
            "reboot" => commands::cmd_reboot(),
            "halt" => commands::cmd_halt(),
            "clear" => commands::cmd_clear(),
            "echo" => commands::cmd_echo(args),
            "calc" => commands::cmd_calc(args),
            "paging" => commands::cmd_paging(),
            "" => {}
            _ => {
                // Check if it looks like a math expression (e.g., "1+1", "10/2")
                if is_math_expr(cmd) {
                    // Build the full expression from cmd + args (in case user typed "1 + 1" with spaces)
                    let mut full_expr = [0u8; 80];
                    let mut len = 0;
                    
                    // Copy cmd
                    for b in cmd.as_bytes().iter() {
                        if len < full_expr.len() {
                            full_expr[len] = *b;
                            len += 1;
                        }
                    }
                    
                    // Add args if any
                    if !args.is_empty() {
                        for b in args.as_bytes().iter() {
                            if len < full_expr.len() {
                                full_expr[len] = *b;
                                len += 1;
                            }
                        }
                    }
                    
                    if let Ok(expr) = core::str::from_utf8(&full_expr[..len]) {
                        commands::cmd_calc(expr);
                    } else {
                        self.unknown_command(cmd);
                    }
                } else {
                    self.unknown_command(cmd);
                }
            }
        }
    }

    fn unknown_command(&self, cmd: &str) {
        print("Unknown command: ", ColorCode::new(Color::LightRed, Color::Black));
        print(cmd, ColorCode::new(Color::LightRed, Color::Black));
        print("\nType 'help' for available commands.\n", ColorCode::new(Color::White, Color::Black));
    }
}

/// Check if a string looks like a math expression (contains +, -, *, /)
fn is_math_expr(s: &str) -> bool {
    let bytes = s.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        // Skip first char (could be negative, though we don't support that)
        if i == 0 {
            continue;
        }
        if b == b'+' || b == b'-' || b == b'*' || b == b'/' {
            return true;
        }
    }
    false
}

pub static mut SHELL: Shell = Shell::new();
