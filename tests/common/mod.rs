#![allow(dead_code)]

//! Loading a film from a folder of chunk files.
//!
//! The Halo Infinite film manifest lists each chunk with an `Index`, a
//! `ChunkType` (1 header, 2 replay, 3 highlights) and a `FileRelativePath`
//! such as `/filmChunk0`. A downloader maps each entry to
//! `Chunk::new(entry.Index, ChunkKind::from_manifest(entry.ChunkType)?, bytes)`.
//! Without the manifest, a folder of `filmChunk{N}` files (as downloaded) or
//! `filmChunk{N}_dec` files (inflated) gives the kinds by position.

use filmshell::{Chunk, ChunkKind};
use std::path::Path;

/// The chunks of the film in `dir`.
pub fn load_folder(dir: &Path) -> std::io::Result<Vec<Chunk<'static>>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let Some(rest) = name.strip_prefix("filmChunk") else { continue };
        let Ok(index) = rest.trim_end_matches("_dec").parse::<u32>() else { continue };
        // Prefer the inflated copy when both are there.
        if let Some(i) = found.iter().position(|(n, _): &(u32, _)| *n == index) {
            if name.ends_with("_dec") {
                found[i] = (index, path);
            }
            continue;
        }
        found.push((index, path));
    }
    let count = found.len() as u32;
    found
        .into_iter()
        .map(|(index, path)| Ok(Chunk::new(index, ChunkKind::by_position(index, count), std::fs::read(path)?)))
        .collect()
}

/// `data` as a zlib stream of stored blocks, as a downloaded chunk is
/// compressed (with real deflate there).
#[allow(dead_code)]
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = if data.is_empty() { vec![&[][..]] } else { data.chunks(0xffff).collect() };
    for (i, b) in blocks.iter().enumerate() {
        out.push(u8::from(i + 1 == blocks.len()));
        let n = b.len() as u16;
        out.extend(n.to_le_bytes());
        out.extend((!n).to_le_bytes());
        out.extend_from_slice(b);
    }
    let (mut a, mut s) = (1u32, 0u32);
    for &x in data {
        a = (a + x as u32) % 65521;
        s = (s + a) % 65521;
    }
    out.extend(((s << 16) | a).to_be_bytes());
    out
}
