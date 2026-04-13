use core::arch::asm;

#[allow(dead_code)]
fn are_interrupts_enabled() -> bool {
    let flag: u32;
    unsafe {
        asm!(
            "pushf",         // Push EFLAGS register onto the stack
            "pop {0}",       // Pop EFLAGS into `flag`
            out(reg) flag
        );
    }

    // Check if the Interrupt Flag (bit 9) is set
    (flag & (1 << 9)) != 0
}

pub(crate) fn enable_interrupts() {
    unsafe {
        asm!("sti", options(nostack, preserves_flags)); // Enable interrupts
    }
}
