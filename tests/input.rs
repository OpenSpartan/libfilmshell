//! Chunk validation, with no film needed.

use filmshell::{Chunk, ChunkKind, Error, Film};

fn chunk(index: u32, kind: ChunkKind) -> Chunk<'static> {
    Chunk::new(index, kind, vec![0u8; 16])
}

#[test]
fn no_chunks() {
    assert_eq!(Film::from_chunks(Vec::new()).unwrap_err(), Error::NoChunks);
}

#[test]
fn gaps_and_repeats_are_named() {
    let gap = vec![chunk(0, ChunkKind::Header), chunk(3, ChunkKind::Replay), chunk(1, ChunkKind::Replay)];
    assert_eq!(Film::from_chunks(gap).unwrap_err(), Error::MissingChunks(vec![2]));
    let twice = vec![chunk(0, ChunkKind::Header), chunk(1, ChunkKind::Replay), chunk(1, ChunkKind::Replay)];
    assert_eq!(Film::from_chunks(twice).unwrap_err(), Error::DuplicateChunk(1));
}

#[test]
fn kinds_must_fit_their_place() {
    let header_late = vec![chunk(0, ChunkKind::Replay), chunk(1, ChunkKind::Header)];
    assert_eq!(
        Film::from_chunks(header_late).unwrap_err(),
        Error::UnexpectedKind { index: 0, kind: ChunkKind::Replay }
    );
    let highlights_early =
        vec![chunk(0, ChunkKind::Header), chunk(1, ChunkKind::Highlights), chunk(2, ChunkKind::Replay)];
    assert_eq!(
        Film::from_chunks(highlights_early).unwrap_err(),
        Error::UnexpectedKind { index: 1, kind: ChunkKind::Highlights }
    );
    assert_eq!(Film::from_chunks(vec![chunk(0, ChunkKind::Header)]).unwrap_err(), Error::NoReplay);
}

#[test]
fn a_header_that_isnt_one() {
    let chunks = vec![chunk(0, ChunkKind::Header), chunk(1, ChunkKind::Replay)];
    assert_eq!(Film::from_chunks(chunks).unwrap_err(), Error::UnknownHeader);
}

#[test]
fn a_broken_stream_fails_to_inflate() {
    let bad = Chunk::new(0, ChunkKind::Header, vec![0x78, 0x9c, 0xff, 0xff, 0xff]);
    assert!(bad.is_compressed());
    assert_eq!(Film::from_chunks(vec![bad, chunk(1, ChunkKind::Replay)]).unwrap_err(), Error::Inflate { index: 0 });
}

#[test]
fn manifest_types() {
    assert_eq!(ChunkKind::from_manifest(1), Some(ChunkKind::Header));
    assert_eq!(ChunkKind::from_manifest(3).map(ChunkKind::manifest_type), Some(3));
    assert_eq!(ChunkKind::from_manifest(7), None);
    assert_eq!(ChunkKind::by_position(0, 5), ChunkKind::Header);
    assert_eq!(ChunkKind::by_position(2, 5), ChunkKind::Replay);
    assert_eq!(ChunkKind::by_position(4, 5), ChunkKind::Highlights);
}
