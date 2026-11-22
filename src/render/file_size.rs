use std::fmt::Write;

use crate::render::stack_str::StackStr;

const KB: u64 = 1024;
const MB: u64 = KB * KB;
const GB: u64 = MB * KB;

/// `size` octets as a person reads them, rounded: `512 B`, `340 KB`,
/// `2.1 MB`, `1.4 GB`. On the stack, as a frame draws it.
pub fn shown(size: u64) -> StackStr<24> {
    let mut shown = StackStr::new();
    let rounded = |unit: u64| (size + unit / 2) / unit;
    let tenths = |unit: u64| (size * 10 + unit / 2) / unit;
    (if size < KB {
        write!(shown, "{size} B")
    } else if rounded(KB) < KB {
        write!(shown, "{} KB", rounded(KB))
    } else if tenths(MB) < 10 * KB {
        let tenths = tenths(MB);
        write!(shown, "{}.{} MB", tenths / 10, tenths % 10)
    } else {
        let tenths = tenths(GB);
        write!(shown, "{}.{} GB", tenths / 10, tenths % 10)
    })
    .ok();
    shown
}
