pub static LOWER_CARACTER_MAP: [Option<char>; 128] = {
    let mut map = [None; 128];
    // Numbers
    map[0x02] = Some('1');
    map[0x03] = Some('2');
    map[0x04] = Some('3');
    map[0x05] = Some('4');
    map[0x06] = Some('5');
    map[0x07] = Some('6');
    map[0x08] = Some('7');
    map[0x09] = Some('8');
    map[0x0A] = Some('9');
    map[0x0B] = Some('0');
    // Math symbols (main keyboard and numpad)
    map[0x0C] = Some('-');  // Minus (key between 0 and =)
    map[0x0D] = Some('+');  // Plus (= key types + for math; use numpad - for = if needed)
    map[0x35] = Some('/');  // Slash (main keyboard, next to right shift)
    map[0x37] = Some('*');  // Asterisk (numpad * key)
    map[0x4A] = Some('-');  // Numpad minus
    map[0x4E] = Some('+');  // Numpad plus
    // Letters
    map[0x10] = Some('q');
    map[0x11] = Some('w');
    map[0x12] = Some('e');
    map[0x13] = Some('r');
    map[0x14] = Some('t');
    map[0x15] = Some('y');
    map[0x16] = Some('u');
    map[0x17] = Some('i');
    map[0x18] = Some('o');
    map[0x19] = Some('p');
    map[0x1E] = Some('a');
    map[0x1F] = Some('s');
    map[0x20] = Some('d');
    map[0x21] = Some('f');
    map[0x22] = Some('g');
    map[0x23] = Some('h');
    map[0x24] = Some('j');
    map[0x25] = Some('k');
    map[0x26] = Some('l');
    map[0x2C] = Some('z');
    map[0x2D] = Some('x');
    map[0x2E] = Some('c');
    map[0x2F] = Some('v');
    map[0x30] = Some('b');
    map[0x31] = Some('n');
    map[0x32] = Some('m');
    map[0x39] = Some(' ');
    map
};

pub static UPPER_CARACTER_MAP: [Option<char>; 128] = {
    let mut map = [None; 128];
    // Numbers
    map[0x02] = Some('1');
    map[0x03] = Some('2');
    map[0x04] = Some('3');
    map[0x05] = Some('4');
    map[0x06] = Some('5');
    map[0x07] = Some('6');
    map[0x08] = Some('7');
    map[0x09] = Some('8');
    map[0x0A] = Some('9');
    map[0x0B] = Some('0');
    // Math symbols (shifted variants)
    map[0x0C] = Some('_');  // Underscore (shift + minus)
    map[0x0D] = Some('+');  // Plus (shift + equals)
    map[0x35] = Some('?');  // Question mark (shift + slash)
    map[0x37] = Some('*');  // Asterisk
    map[0x4A] = Some('-');  // Numpad minus
    map[0x4E] = Some('+');  // Numpad plus
    // Letters (uppercase)
    map[0x10] = Some('Q');
    map[0x11] = Some('W');
    map[0x12] = Some('E');
    map[0x13] = Some('R');
    map[0x14] = Some('T');
    map[0x15] = Some('Y');
    map[0x16] = Some('U');
    map[0x17] = Some('I');
    map[0x18] = Some('O');
    map[0x19] = Some('P');
    map[0x1E] = Some('A');
    map[0x1F] = Some('S');
    map[0x20] = Some('D');
    map[0x21] = Some('F');
    map[0x22] = Some('G');
    map[0x23] = Some('H');
    map[0x24] = Some('J');
    map[0x25] = Some('K');
    map[0x26] = Some('L');
    map[0x2C] = Some('Z');
    map[0x2D] = Some('X');
    map[0x2E] = Some('C');
    map[0x2F] = Some('V');
    map[0x30] = Some('B');
    map[0x31] = Some('N');
    map[0x32] = Some('M');
    map[0x39] = Some(' ');
    map
};
