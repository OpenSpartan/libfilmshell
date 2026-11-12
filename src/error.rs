use crate::ChunkKind;
use std::fmt;

/// Why a film could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// No chunks were given.
    NoChunks,
    /// Chunks between 0 and the highest index given are missing: the first
    /// missing indices, at most 16.
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
    /// A chunk holds more than [`MAX_CHUNK_BYTES`](crate::MAX_CHUNK_BYTES)
    /// once inflated, or takes the film past
    /// [`MAX_FILM_BYTES`](crate::MAX_FILM_BYTES).
    TooLarge {
        /// The chunk's index.
        index: u32,
    },
    /// Chunk 0 is not a film header this library reads: either not a film's,
    /// or one recorded by a build older than film version 41 (the manifest's
    /// `FilmMajorVersion`), whose chunk 0 has another layout.
    UnknownHeader,
    /// The film has no replay chunk.
    NoReplay,
    /// No replay chunk records a tick: the bytes given as replay chunks
    /// aren't a film's.
    NoTicks,
    /// A file name that isn't a chunk's (`filmChunk{N}` or
    /// `filmChunk{N}_dec`), given to [`Chunk::from_file`](crate::Chunk::from_file).
    NotAChunkFile {
        /// The name.
        name: String,
    },
    /// The bytes of a chunk file are no kind of film chunk:
    /// [`ChunkKind::detect`](crate::ChunkKind::detect) doesn't know them.
    UnknownChunk {
        /// The chunk's index.
        index: u32,
    },
    /// A manifest entry's `ChunkType` is none of a film's
    /// ([`Chunk::from_manifest`](crate::Chunk::from_manifest)).
    UnknownChunkType {
        /// The entry's index.
        index: u32,
        /// The type it gave.
        chunk_type: u32,
    },
    /// A folder or a file in it could not be read
    /// ([`Chunk::read_folder`](crate::Chunk::read_folder)).
    Io {
        /// The folder or file.
        path: std::path::PathBuf,
        /// What went wrong, as [`std::io::Error::kind`] gives it.
        kind: std::io::ErrorKind,
        /// The operating system's message.
        message: String,
    },
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
            Error::UnexpectedKind { index, kind } => write!(f, "chunk {index} cannot be a {kind} chunk"),
            Error::Inflate { index } => write!(f, "chunk {index} does not inflate"),
            Error::TooLarge { index } => write!(f, "chunk {index} is past the size limit for a chunk or a film"),
            Error::UnknownHeader => write!(f, "chunk 0 is not a film header this library reads"),
            Error::NoReplay => write!(f, "the film has no replay chunk"),
            Error::NoTicks => write!(f, "no replay chunk records a tick"),
            Error::NotAChunkFile { name } => write!(f, "{name} is not the name of a film chunk file"),
            Error::UnknownChunk { index } => write!(f, "chunk {index} is not a film chunk"),
            Error::UnknownChunkType { index, chunk_type } => {
                write!(f, "chunk {index} has type {chunk_type}, which is no film chunk type")
            }
            Error::Io { path, message, .. } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for Error {}
