use std::borrow::Cow;

/// What a chunk holds. The film's manifest gives each chunk a type: the
/// header chunk first, then the replay chunks, then the HighlightEvents
/// chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
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

    /// The kind a complete film's chunk `index` has when there are `count`
    /// chunks in all: the first is the header, the last the highlights.
    /// For chunks without a manifest at hand.
    pub fn by_position(index: u32, count: u32) -> ChunkKind {
        if index == 0 {
            ChunkKind::Header
        } else if index + 1 == count {
            ChunkKind::Highlights
        } else {
            ChunkKind::Replay
        }
    }
}

/// The most bytes a compressed chunk may inflate to: 256 MiB, about a
/// hundred times the largest real chunk.
pub const MAX_CHUNK_BYTES: usize = 256 << 20;

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

    /// Whether the bytes are a zlib stream: a deflate method byte with a
    /// valid header check. Inflated chunks never start this way.
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
