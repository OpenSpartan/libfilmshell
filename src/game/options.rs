//! Game variant options: their names, and the defaults the mode scripts
//! give them.
//!
//! A variant stores each option it sets as the murmur3 hash of the option's
//! lower-cased name and an 8-byte value. The names come from the option
//! scripts the game ships.

use super::options_data::{MODES, OPTION_NAMES};

/// An option's value.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum OptionValue {
    /// A number.
    Number(f64),
    /// A flag.
    Bool(bool),
}

impl std::fmt::Display for OptionValue {
    /// A number as it is (`3`, `0.5`), a flag as `true` or `false`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OptionValue::Number(n) => write!(f, "{n}"),
            OptionValue::Bool(b) => write!(f, "{b}"),
        }
    }
}

/// One option of a mode's Options script.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ModeOption {
    /// The name variants store it under.
    pub name: &'static str,
    /// Every name a variant may store it under.
    pub keys: &'static [&'static str],
    /// The default the mode's scripts give it, when that is a literal.
    pub default: Option<OptionValue>,
}

/// A mode's Options script and its options.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ModeOptions {
    /// The options' name prefix ("CTFOptions").
    pub prefix: &'static str,
    /// The Options script's file name, lower-cased.
    pub script: &'static str,
    /// Its options, in the script's order.
    pub options: &'static [ModeOption],
}

/// The option whose lower-cased name hashes to `hash`.
pub fn option_name(hash: u32) -> Option<&'static str> {
    OPTION_NAMES.binary_search_by_key(&hash, |o| o.0).ok().map(|i| OPTION_NAMES[i].1)
}

/// Every mode's options.
pub fn modes() -> &'static [ModeOptions] {
    MODES
}

/// The murmur3 (x86, 32-bit, seed 0) of `data`: how the game hashes names
/// into string ids.
pub fn murmur3(data: &[u8]) -> u32 {
    let (c1, c2) = (0xcc9e_2d51u32, 0x1b87_3593u32);
    let mut h = 0u32;
    let chunks = data.chunks_exact(4);
    let tail = chunks.remainder();
    for w in chunks {
        let k = u32::from_le_bytes([w[0], w[1], w[2], w[3]]).wrapping_mul(c1).rotate_left(15).wrapping_mul(c2);
        h = (h ^ k).rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    if !tail.is_empty() {
        let k = tail.iter().enumerate().fold(0u32, |k, (i, &b)| k | (b as u32) << (8 * i));
        h ^= k.wrapping_mul(c1).rotate_left(15).wrapping_mul(c2);
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_hash_to_their_ids() {
        assert!(OPTION_NAMES.windows(2).all(|w| w[0].0 < w[1].0));
        for &(hash, name) in OPTION_NAMES {
            assert_eq!(murmur3(name.to_ascii_lowercase().as_bytes()), hash, "{name}");
        }
        assert_eq!(murmur3(b"default"), 0x42c9679f);
        assert!(modes().iter().any(|m| m.script == "ctfoptions.lua" && m.options.len() >= 10));
    }
}
