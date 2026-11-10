//! The reference films for unit tests, found as `tests/common` finds them:
//! the folder `FILMSHELL_TEST_FILMS` names, else `films/` here. Tests that
//! need a film return early without films, and fail without one when the
//! variable is set.

use std::path::{Path, PathBuf};

const VAR: &str = "FILMSHELL_TEST_FILMS";

/// Every film's folder, sorted.
pub fn film_dirs() -> Vec<PathBuf> {
    let root = match std::env::var_os(VAR) {
        Some(root) => PathBuf::from(root),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).join("films"),
    };
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("filmChunk0").exists() || p.join("filmChunk0_dec").exists())
        .collect();
    assert!(!dirs.is_empty() || std::env::var_os(VAR).is_none(), "{VAR} is set, but {} holds no films", root.display());
    dirs.sort();
    dirs
}

/// The inflated chunks, in index order, of the film whose match id starts
/// with `prefix`.
pub fn chunks(prefix: &str) -> Option<Vec<Vec<u8>>> {
    let dir = film_dirs().into_iter().find(|d| d.file_name().is_some_and(|n| n.to_string_lossy().starts_with(prefix)));
    assert!(dir.is_some() || std::env::var_os(VAR).is_none(), "the test films have no {prefix}");
    let dir = dir?;
    let mut out = Vec::new();
    while let Some(b) = chunk(&dir, out.len()) {
        out.push(b);
    }
    (!out.is_empty()).then_some(out)
}

/// Chunk `index` of the film in `dir`, inflated: the downloaded file, else
/// the inflated copy.
pub fn chunk(dir: &Path, index: usize) -> Option<Vec<u8>> {
    match std::fs::read(dir.join(format!("filmChunk{index}"))) {
        Ok(b) => Some(
            crate::inflate::zlib(&b, crate::MAX_CHUNK_BYTES)
                .unwrap_or_else(|e| panic!("{}: chunk {index}: {e:?}", dir.display())),
        ),
        Err(_) => std::fs::read(dir.join(format!("filmChunk{index}_dec"))).ok(),
    }
}
