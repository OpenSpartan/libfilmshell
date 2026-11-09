#![allow(dead_code)]

//! Loading a film from a folder of chunk files.
//!
//! The Halo Infinite film manifest lists each chunk with an `Index`, a
//! `ChunkType` (1 header, 2 replay, 3 highlights) and a `FileRelativePath`
//! such as `/filmChunk0`. A downloader maps each entry to
//! `Chunk::new(entry.Index, ChunkKind::from_manifest(entry.ChunkType)?, bytes)`.
//! Without the manifest, `Chunk::from_file` reads the index from a
//! `filmChunk{N}` (as downloaded) or `filmChunk{N}_dec` (inflated) file name
//! and the kind from the bytes.

use filmshell::Chunk;
use std::io::{Error, ErrorKind};
use std::path::Path;

/// The chunks of the film in `dir`, the inflated copy of a chunk where both
/// are there.
pub fn load_folder(dir: &Path) -> std::io::Result<Vec<Chunk<'static>>> {
    let mut chunks = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.starts_with("filmChunk") || dir.join(format!("{name}_dec")).exists() {
            continue;
        }
        let chunk = Chunk::from_file(name, std::fs::read(&path)?)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, format!("{name} is not a film chunk")))?;
        chunks.push(chunk);
    }
    Ok(chunks)
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
