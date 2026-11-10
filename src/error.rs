use crate::ChunkKind;
use std::fmt;

/// Why a film could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// No chunks were given.
    NoChunks,
    /// Chunks between 0 and the highest index given are missing.
    MissingChunks(Vec<u32>),
    /// Two chunks have the same index.
    DuplicateChunk(u32),
    /// A chunk's kind doesn't fit its place: the header must be chunk 0,
    /// the highlights the last chunk, and every other chunk a replay chunk.
    UnexpectedKind {
        /// The chunk's index.
        index: u32,
        /// The kind it was given.
        kind: ChunkKind,
    },
    /// A compressed chunk did not inflate.
    Inflate {
        /// The chunk's index.
        index: u32,
    },
    /// A compressed chunk inflates to more than [`MAX_CHUNK_BYTES`](crate::MAX_CHUNK_BYTES).
    TooLarge {
        /// The chunk's index.
        index: u32,
    },
    /// Chunk 0 is not a film header the library knows.
    UnknownHeader,
    /// The film has no replay chunk.
    NoReplay,
    /// No replay chunk records a tick: the bytes given as replay chunks
    /// aren't a film's.
    NoTicks,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NoChunks => write!(f, "no chunks given"),
            Error::MissingChunks(m) => {
                let list: Vec<String> = m.iter().map(u32::to_string).collect();
                write!(f, "missing chunks {}", list.join(", "))
            }
            Error::DuplicateChunk(i) => write!(f, "chunk {i} given twice"),
            Error::UnexpectedKind { index, kind } => write!(f, "chunk {index} cannot be a {kind:?} chunk"),
            Error::Inflate { index } => write!(f, "chunk {index} does not inflate"),
            Error::TooLarge { index } => {
                write!(f, "chunk {index} inflates to more than {} bytes", crate::MAX_CHUNK_BYTES)
            }
            Error::UnknownHeader => write!(f, "chunk 0 is not a film header"),
            Error::NoReplay => write!(f, "the film has no replay chunk"),
            Error::NoTicks => write!(f, "no replay chunk records a tick"),
        }
    }
}

impl std::error::Error for Error {}
