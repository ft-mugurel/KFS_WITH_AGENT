use crate::x86::io::outb;

pub(crate) unsafe fn init_pic() {
    // Master PIC: Başlangıç komutları
    outb(0x20, 0x11); // ICW1: Başlangıç
    outb(0x21, 0x20); // ICW2: Master PIC için vektör offset'i 0x20 (32)
    outb(0x21, 0x04); // ICW3: Slave PIC'in IRQ2'ye bağlandığını belirt
    outb(0x21, 0x01); // ICW4: 8086 modunu ayarla

    // Slave PIC: Başlangıç komutları
    outb(0xA0, 0x11); // ICW1: Başlangıç
    outb(0xA1, 0x28); // ICW2: Slave PIC için vektör offset'i 0x28 (40)
    outb(0xA1, 0x02); // ICW3: Slave PIC'in IRQ2'ye bağlandığını belirt
    outb(0xA1, 0x01); // ICW4: 8086 modunu ayarla

    // Tüm interruptları maskeler (engeller) - Başlangıçta maskeyi kaldırıyoruz
    outb(0x21, 0xFF); // Master PIC'teki tüm interruptları maskeler
    outb(0xA1, 0xFF); // Slave PIC'teki tüm interruptları maskeler

    // Klavye interrupt'ını (IRQ1) etkinleştir
    outb(0x21, 0xFD); // Master PIC: IRQ1 (klavye) için maskeyi kaldır
    outb(0xA1, 0xFF); // Slave PIC'teki interruptları maskele (gerekirse)
}

