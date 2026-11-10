//! Loading the local films for unit tests. `films/` is local
//! only; tests that need it return early without it.

use std::path::PathBuf;

/// The inflated chunks of the film whose folder name starts with `prefix`.
pub fn chunks(prefix: &str) -> Option<Vec<Vec<u8>>> {
    let films = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("films");
    let dir =
        std::fs::read_dir(films).ok()?.flatten().find(|d| d.file_name().to_string_lossy().starts_with(prefix))?.path();
    let mut out = Vec::new();
    while let Ok(b) = std::fs::read(dir.join(format!("filmChunk{}_dec", out.len()))) {
        out.push(b);
    }
    (!out.is_empty()).then_some(out)
}
