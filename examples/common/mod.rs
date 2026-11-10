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
use std::path::Path;

/// The chunks of the film in `dir`, the inflated copy of a chunk where both
/// are there. Other files are skipped.
pub fn load_folder(dir: &Path) -> Result<Vec<Chunk<'static>>, Box<dyn std::error::Error>> {
    let mut chunks = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if Chunk::file_index(name).is_none() || dir.join(format!("{name}_dec")).exists() {
            continue;
        }
        chunks.push(Chunk::from_file(name, std::fs::read(&path)?)?);
    }
    Ok(chunks)
}
