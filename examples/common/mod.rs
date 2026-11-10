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
/// are there. Other files are skipped.
pub fn load_folder(dir: &Path) -> std::io::Result<Vec<Chunk<'static>>> {
    let mut chunks = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !is_chunk_name(name) || dir.join(format!("{name}_dec")).exists() {
            continue;
        }
        let chunk = Chunk::from_file(name, std::fs::read(&path)?)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, format!("{name} is not a film chunk")))?;
        chunks.push(chunk);
    }
    Ok(chunks)
}

/// `filmChunk{N}` or `filmChunk{N}_dec`.
fn is_chunk_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("filmChunk") else { return false };
    let digits = rest.strip_suffix("_dec").unwrap_or(rest);
    !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit())
}
