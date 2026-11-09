//! Facts from chunk 0's header, after the component registry: the game build that
//! recorded the film, the film's start on the wall clock and the map
//! variant's ids.
//!
//! ```text
//! 0x0       [4] [4]  then the registry (0xcb200 bytes)
//! 0xcb208   123 u32s  event versions
//! 0xcb3f4   [32] build ("6.10026.18411.0")  [32] branch ("HI_1_13_0")  [32] "release"
//! 0xcb454   [4] [4]  build-dependent numbers
//! 0xcb45c   [1]  then, a bit off the bytes:
//!           [256] [256]  [4] start, Unix seconds (LE)  [12]  [4096] [4096] [4096]
//!           [16] map variant AssetId  [16] VersionId (GUIDs, little-endian fields)
//! ```
//!
//! The start is the only wall clock in a film, some seconds after the
//! match's own start. The ids are the map variant's as the Halo services
//! name it.

use crate::bits::bits_at;

const BUILD: usize = 0xcb3f4;
const BRANCH: usize = 0xcb414;
/// The bit the shifted block starts on.
const BLOCK_BIT: i64 = 8 * 0xcb45c + 1;
const START_BYTE: i64 = 512;
const ASSET_BYTE: i64 = 512 + 4 + 12 + 3 * 4096;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    pub build: Option<String>,
    pub branch: Option<String>,
    /// The film's start, Unix seconds.
    pub start: Option<u32>,
    pub map_asset_id: Option<[u8; 16]>,
    pub map_version_id: Option<[u8; 16]>,
    /// The map variant's `levl` tag id.
    pub level: Option<u32>,
}

/// The bit of chunk 0 that holds the map variant's `levl` tag id, 32 bits
/// big-endian, in the region the game variant's Bond data shares (3 bits off
/// the bytes).
const LEVEL_BIT: i64 = 8 * 0xce6a2 + 3;

/// A NUL-padded 32-byte string of printable ASCII; None otherwise.
fn text(c: &[u8], at: usize) -> Option<String> {
    let b = c.get(at..at + 32)?;
    let end = b.iter().position(|&x| x == 0).unwrap_or(32);
    let s = &b[..end];
    (!s.is_empty() && s.iter().all(|x| x.is_ascii_graphic())).then(|| String::from_utf8_lossy(s).into_owned())
}

/// `n` bytes from bit `at`.
fn bytes(c: &[u8], at: i64, n: usize) -> Vec<u8> {
    (0..n as i64).map(|k| bits_at(c, at + 8 * k, 8) as u8).collect()
}

/// A GUID's 16 bytes, None for all zeros.
fn guid(b: Vec<u8>) -> Option<[u8; 16]> {
    let g: [u8; 16] = b.try_into().ok()?;
    g.iter().any(|&x| x != 0).then_some(g)
}

/// The header's facts. Nothing when the build string isn't where this
/// layout puts it (another film version).
pub fn read(chunk0: &[u8]) -> Header {
    let Some(build) = text(chunk0, BUILD).filter(|s| s.bytes().all(|x| x.is_ascii_digit() || x == b'.')) else {
        return Header::default();
    };
    if chunk0.len() < (BLOCK_BIT / 8 + ASSET_BYTE + 40) as usize {
        return Header { build: Some(build), ..Header::default() };
    }
    let start = u32::from_le_bytes(bytes(chunk0, BLOCK_BIT + 8 * START_BYTE, 4).try_into().unwrap());
    Header {
        build: Some(build),
        branch: text(chunk0, BRANCH),
        start: (start > 0).then_some(start),
        map_asset_id: guid(bytes(chunk0, BLOCK_BIT + 8 * ASSET_BYTE, 16)),
        map_version_id: guid(bytes(chunk0, BLOCK_BIT + 8 * (ASSET_BYTE + 16), 16)),
        level: Some(bits_at(chunk0, LEVEL_BIT, 32)).filter(|&l| l != 0 && l != u32::MAX),
    }
}
