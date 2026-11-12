use crate::Error;
use std::borrow::Cow;
use std::path::Path;

/// What a chunk holds. The film's manifest gives each chunk a type: the
/// header chunk first, then the replay chunks, then the `HighlightEvents`
/// chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum ChunkKind {
    /// Chunk 0: the component registry, the header, the game and map
    /// variants and the roster. Manifest chunk type 1.
    Header,
    /// A stretch of the match: packets of tick updates, player structs and
    /// seek snapshots. Manifest chunk type 2.
    Replay,
    /// The theater timeline: kills, deaths and medals with millisecond
    /// times. Manifest chunk type 3, the last chunk.
    Highlights,
}

impl ChunkKind {
    /// The kind of a manifest's `ChunkType`.
    pub fn from_manifest(chunk_type: u32) -> Option<ChunkKind> {
        match chunk_type {
            1 => Some(ChunkKind::Header),
            2 => Some(ChunkKind::Replay),
            3 => Some(ChunkKind::Highlights),
            _ => None,
        }
    }

    /// The manifest's `ChunkType` for this kind.
    pub fn manifest_type(self) -> u32 {
        match self {
            ChunkKind::Header => 1,
            ChunkKind::Replay => 2,
            ChunkKind::Highlights => 3,
        }
    }

    /// The kind of chunk `index` from its inflated bytes, for chunks without
    /// a manifest at hand. Chunk 0 is the header. The others are packets
    /// (`format`, with the `raw` feature, has their layout) that end on the chunk's
    /// last byte: a replay chunk's first packet is kind 1, the highlights
    /// chunk's kind 9. None for bytes that are neither.
    pub fn detect(index: u32, inflated: &[u8]) -> Option<ChunkKind> {
        if index == 0 {
            return Some(ChunkKind::Header);
        }
        match crate::container::walk_packets(inflated)?.first()?.kind {
            REPLAY_FIRST => Some(ChunkKind::Replay),
            HIGHLIGHTS_FIRST => Some(ChunkKind::Highlights),
            _ => None,
        }
    }
}

impl std::fmt::Display for ChunkKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ChunkKind::Header => "header",
            ChunkKind::Replay => "replay",
            ChunkKind::Highlights => "highlights",
        })
    }
}

/// The type of a replay chunk's first packet.
const REPLAY_FIRST: u16 = 1;
/// The type of the highlights chunk's first packet.
const HIGHLIGHTS_FIRST: u16 = 9;

/// The most bytes one chunk may hold once inflated: 256 MiB, about a hundred
/// times the largest chunk of the test films.
pub const MAX_CHUNK_BYTES: usize = 256 << 20;

/// The most bytes a film's chunks may hold together once inflated: 1 GiB.
///
/// That is fourteen times the largest of the test films (72 MB, 35 players over
/// 18 minutes). Reading a film takes about four times its inflated size in
/// memory at its peak: 300 MB for that one.
pub const MAX_FILM_BYTES: usize = 1 << 30;

/// One chunk of a film: its index in the manifest, its kind, and its bytes,
/// either as downloaded (zlib) or already inflated.
#[derive(Clone, Debug)]
pub struct Chunk<'a> {
    /// The chunk's index, 0 for the header.
    pub index: u32,
    /// What it holds.
    pub kind: ChunkKind,
    /// Its bytes, compressed or not.
    pub data: Cow<'a, [u8]>,
}

impl<'a> Chunk<'a> {
    /// A chunk from borrowed or owned bytes.
    pub fn new(index: u32, kind: ChunkKind, data: impl Into<Cow<'a, [u8]>>) -> Chunk<'a> {
        Chunk { index, kind, data: data.into() }
    }

    /// A chunk from an entry of the film's manifest: its `Index`, its
    /// `ChunkType` (1 header, 2 replay, 3 highlights) and the bytes at its
    /// `FileRelativePath`, compressed as downloaded or already inflated.
    ///
    /// # Errors
    ///
    /// [`Error::UnknownChunkType`] for a `ChunkType` other than 1, 2 and 3.
    pub fn from_manifest(index: u32, chunk_type: u32, data: impl Into<Cow<'a, [u8]>>) -> Result<Chunk<'a>, Error> {
        let kind = ChunkKind::from_manifest(chunk_type).ok_or(Error::UnknownChunkType { index, chunk_type })?;
        Ok(Chunk::new(index, kind, data))
    }

    /// A chunk saved as a file: `filmChunk{N}` as downloaded or
    /// `filmChunk{N}_dec` inflated, `name` the file's name without its
    /// folder. The index comes from the name and the kind from the bytes
    /// ([`ChunkKind::detect`]); compressed bytes are inflated here.
    ///
    /// [`Chunk::read_folder`] reads a folder of them.
    ///
    /// ```no_run
    /// use filmshell::Chunk;
    ///
    /// let header = Chunk::from_file("filmChunk0", std::fs::read("path/to/film/filmChunk0")?)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::NotAChunkFile`] for a name that isn't a chunk file's;
    /// [`Error::Inflate`] or [`Error::TooLarge`] for compressed bytes that
    /// don't inflate or inflate past [`MAX_CHUNK_BYTES`];
    /// [`Error::UnknownChunk`] for bytes that are no chunk of a film. Chunk
    /// 0's bytes aren't checked here: a bad one fails in
    /// [`Film::from_chunks`](crate::Film::from_chunks) as
    /// [`Error::UnknownHeader`].
    pub fn from_file(name: &str, data: impl Into<Cow<'a, [u8]>>) -> Result<Chunk<'a>, Error> {
        let index = Chunk::file_index(name).ok_or_else(|| Error::NotAChunkFile { name: name.to_string() })?;
        let mut data = data.into();
        if is_zlib(&data) {
            let inflated = crate::inflate::zlib(&data, MAX_CHUNK_BYTES).map_err(|f| match f {
                crate::inflate::Fault::Malformed => Error::Inflate { index },
                crate::inflate::Fault::TooLong => Error::TooLarge { index },
            })?;
            data = Cow::Owned(inflated);
        }
        let kind = ChunkKind::detect(index, &data).ok_or(Error::UnknownChunk { index })?;
        Ok(Chunk { index, kind, data })
    }

    /// Every chunk file in folder `dir`, read with [`Chunk::from_file`] in
    /// index order: the inflated copy of a chunk where both are there. Other
    /// files are skipped.
    ///
    /// ```no_run
    /// use filmshell::{Chunk, Film, Hints};
    ///
    /// let chunks = Chunk::read_folder("path/to/film")?;
    /// let film = Film::from_chunks_with(chunks, &Hints::default())?;
    /// # Ok::<(), filmshell::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`Error::Io`] for a folder or file that can't be read, and the errors
    /// of [`Chunk::from_file`].
    pub fn read_folder(dir: impl AsRef<Path>) -> Result<Vec<Chunk<'static>>, Error> {
        let dir = dir.as_ref();
        let io = |path: &Path, e: std::io::Error| Error::Io {
            path: path.to_path_buf(),
            kind: e.kind(),
            message: e.to_string(),
        };
        let mut chunks = Vec::new();
        for entry in std::fs::read_dir(dir).map_err(|e| io(dir, e))? {
            let path = entry.map_err(|e| io(dir, e))?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            if Chunk::file_index(name).is_none() || dir.join(format!("{name}_dec")).exists() {
                continue;
            }
            let data = std::fs::read(&path).map_err(|e| io(&path, e))?;
            chunks.push(Chunk::from_file(name, data)?);
        }
        chunks.sort_by_key(|c| c.index);
        Ok(chunks)
    }

    /// The index a chunk file's name gives: `N` for `filmChunk{N}` (as
    /// downloaded) or `filmChunk{N}_dec` (inflated). None for any other name.
    pub fn file_index(name: &str) -> Option<u32> {
        let digits = name.strip_prefix("filmChunk")?;
        let digits = digits.strip_suffix("_dec").unwrap_or(digits);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        digits.parse().ok()
    }

    /// Whether the bytes are a zlib stream: a deflate method byte with a
    /// valid header check. Replay and highlights chunks can't start this way
    /// once inflated, and no test film's header does.
    pub fn is_compressed(&self) -> bool {
        is_zlib(&self.data)
    }
}

pub(crate) fn is_zlib(b: &[u8]) -> bool {
    match b {
        [cmf, flg, ..] => cmf & 0x0f == 8 && cmf >> 4 <= 7 && ((*cmf as u16) << 8 | *flg as u16) % 31 == 0,
        _ => false,
    }
}
