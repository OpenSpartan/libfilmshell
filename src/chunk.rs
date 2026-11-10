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

    /// The kind of chunk `index` from its inflated bytes, for chunks without
    /// a manifest at hand. Chunk 0 is the header. The others are packets,
    /// each a 16-byte head (a 16-bit type, a 32-bit size, a timestamp) and
    /// its payload, that end on the chunk's last byte: a replay chunk's
    /// first packet is type 1, the highlights chunk's type 9. None for bytes
    /// that are neither.
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

/// The type of a replay chunk's first packet.
const REPLAY_FIRST: u16 = 1;
/// The type of the highlights chunk's first packet.
const HIGHLIGHTS_FIRST: u16 = 9;

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

    /// A chunk saved as a file: `filmChunk{N}` as downloaded or
    /// `filmChunk{N}_dec` inflated, `name` the file's name without its
    /// folder. The index comes from the name and the kind from the bytes
    /// ([`ChunkKind::detect`]); compressed bytes are inflated here. None
    /// when the name isn't a chunk's or the bytes are no chunk of a film.
    pub fn from_file(name: &str, data: impl Into<Cow<'a, [u8]>>) -> Option<Chunk<'a>> {
        let digits = name.strip_prefix("filmChunk")?;
        let digits = digits.strip_suffix("_dec").unwrap_or(digits);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let index: u32 = digits.parse().ok()?;
        let mut data = data.into();
        if is_zlib(&data) {
            data = Cow::Owned(crate::inflate::zlib(&data, MAX_CHUNK_BYTES).ok()?);
        }
        let kind = ChunkKind::detect(index, &data)?;
        Some(Chunk { index, kind, data })
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
