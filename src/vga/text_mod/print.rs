#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ({
        use crate::vga::text_mod::out::{print, ColorCode, Color};
        let _ = writeln!(&mut io::stdout(), $($arg)*);
    });
}

