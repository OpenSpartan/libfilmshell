//! The script file names chunk 0 carries.

use std::collections::BTreeSet;

/// Script names the 3-bit-shifted chunk 0 carries, lower-cased: each is a
/// path ending in `.lua`.
pub fn script_names(chunk0: &[u8]) -> BTreeSet<String> {
    let mut s = vec![0u8; chunk0.len().saturating_sub(1)];
    for (i, b) in s.iter_mut().enumerate() {
        *b = (chunk0[i] << 3) | (chunk0[i + 1] >> 5);
    }
    let name_char = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b' ';
    let mut out = BTreeSet::new();
    for end in (4..s.len()).filter(|&e| &s[e - 4..e] == b".lua") {
        let mut start = end - 4;
        while start > 0 && name_char(s[start - 1]) {
            start -= 1;
        }
        if start > 0 && (s[start - 1] == b'\\' || s[start - 1] == b'/') && start < end - 4 {
            if let Ok(n) = std::str::from_utf8(&s[start..end]) {
                out.insert(n.to_ascii_lowercase());
            }
        }
    }
    out
}
