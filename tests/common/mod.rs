#![allow(dead_code)]

//! The reference films the tests read.
//!
//! They live outside this repository. `FILMSHELL_TEST_FILMS` names the folder
//! that holds them, one folder per film; without it the tests look in
//! `films/` here, and with neither they check nothing. When the variable is
//! set the films must be there, so a missing or empty folder fails the tests
//! instead of letting them pass.
//!
//! A film's folder holds its chunks as downloaded (`filmChunk{N}`, zlib) or
//! inflated (`filmChunk{N}_dec`), and its manifest, `film-metadata.json`:
//! each chunk's `Index`, `ChunkType` (1 header, 2 replay, 3 highlights) and
//! `FileRelativePath`.

use filmshell::{Chunk, ChunkKind};
use std::io::{Error, ErrorKind};
use std::path::{Path, PathBuf};

const VAR: &str = "FILMSHELL_TEST_FILMS";

/// Every film's folder, sorted.
pub fn film_dirs() -> Vec<PathBuf> {
    let Some(root) = std::env::var_os(VAR) else {
        return films_in(&Path::new(env!("CARGO_MANIFEST_DIR")).join("films"));
    };
    let dirs = films_in(Path::new(&root));
    assert!(!dirs.is_empty(), "{VAR} is set, but {} holds no films", Path::new(&root).display());
    dirs
}

/// The folder of the film whose match id starts with `prefix`. None when
/// there are no films; with `FILMSHELL_TEST_FILMS` set, a film it lacks
/// fails the test.
pub fn film(prefix: &str) -> Option<PathBuf> {
    let found =
        film_dirs().into_iter().find(|d| d.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)));
    assert!(found.is_some() || std::env::var_os(VAR).is_none(), "the test films have no {prefix}");
    found
}

fn films_in(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("filmChunk0").exists() || p.join("filmChunk0_dec").exists())
        .collect();
    dirs.sort();
    dirs
}

/// The film's chunks as a downloader hands them over: one per manifest
/// entry, its kind from the entry's chunk type and its bytes as they are on
/// disk, compressed when the file is the downloaded one.
pub fn load(dir: &Path) -> Vec<Chunk<'static>> {
    let manifest = std::fs::read(dir.join("film-metadata.json")).expect("a film manifest");
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).expect("a film manifest");
    let entries = manifest["CustomData"]["Chunks"].as_array().expect("a chunk list");
    entries
        .iter()
        .map(|e| {
            let index = e["Index"].as_u64().expect("an index") as u32;
            let kind = ChunkKind::from_manifest(e["ChunkType"].as_u64().expect("a chunk type") as u32)
                .expect("a known chunk type");
            let path = dir.join(e["FileRelativePath"].as_str().expect("a file").trim_start_matches('/'));
            let bytes = std::fs::read(&path)
                .or_else(|_| std::fs::read(dir.join(format!("filmChunk{index}_dec"))))
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            Chunk::new(index, kind, bytes)
        })
        .collect()
}

/// The chunks of the film in `dir` read by their file names alone, with
/// [`Chunk::from_file`], which inflates them. The inflated copy wins where
/// both are there.
pub fn load_folder(dir: &Path) -> std::io::Result<Vec<Chunk<'static>>> {
    let mut chunks = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.starts_with("filmChunk") || dir.join(format!("{name}_dec")).exists() {
            continue;
        }
        let chunk = Chunk::from_file(name, std::fs::read(&path)?).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        chunks.push(chunk);
    }
    Ok(chunks)
}

/// `data` as a zlib stream of stored blocks, as a downloaded chunk is
/// compressed (with real deflate there).
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
