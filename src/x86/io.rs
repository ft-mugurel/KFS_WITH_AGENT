use core::arch::asm;

/// Write 8 bits to port
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn outb(port: u16, val: u8) {
    asm!("outb %al, %dx", in("al") val, in("dx") port, options(att_syntax));
}

/// Read 8 bits from port
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn inb(port: u16) -> u8 {
    let ret: u8;
    asm!("inb %dx, %al", in("dx") port, out("al") ret, options(att_syntax));
    ret
}

/// Write 16 bits to port
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn outw(port: u16, val: u16) {
    asm!("outw %ax, %dx", in("ax") val, in("dx") port, options(att_syntax));
}

/// Read 16 bits from port
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn inw(port: u16) -> u16 {
    let ret: u16;
    asm!("inw %dx, %ax", in("dx") port, out("ax") ret, options(att_syntax));
    ret
}

/// Write 32 bits to port
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn outl(port: u16, val: u32) {
    asm!("outl %eax, %dx", in("eax") val, in("dx") port, options(att_syntax));
}

/// Read 32 bits from port
///
/// # Safety
/// Needs IO privileges.
#[inline]
pub unsafe fn inl(port: u16) -> u32 {
    let ret: u32;
    asm!("inl %dx, %eax", out("eax") ret, in("dx") port, options(att_syntax));
    ret
}
