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

/// A deflate stream (fixed codes) of a zero byte and then `copies` copies of
/// the 258 bytes before, in a zlib frame with no checksum: it inflates to
/// `1 + 258 * copies` bytes.
fn bomb(copies: usize) -> Vec<u8> {
    let mut bits: Vec<bool> = Vec::new();
    // Huffman codes go most significant bit first, everything else least.
    let code = |bits: &mut Vec<bool>, c: u32, n: u32| bits.extend((0..n).rev().map(|i| c >> i & 1 == 1));
    bits.extend([true, true, false]); // last block, fixed codes
    code(&mut bits, 0x30, 8); // literal 0
    for _ in 0..copies {
        code(&mut bits, 0xc5, 8); // length 258
        code(&mut bits, 0, 5); // distance 1
    }
    code(&mut bits, 0, 7); // end of block
    let mut out = vec![0x78, 0x01];
    out.extend(bits.chunks(8).map(|b| b.iter().enumerate().fold(0u8, |v, (i, &x)| v | (x as u8) << i)));
    out.extend([0; 4]);
    out
}

#[test]
fn a_chunk_that_inflates_without_bound_is_refused() {
    let copies = filmshell::MAX_CHUNK_BYTES / 258 + 1;
    let chunks = vec![chunk(0, ChunkKind::Header), Chunk::new(1, ChunkKind::Replay, bomb(copies))];
    assert!(chunks[1].is_compressed());
    assert_eq!(Film::from_chunks(chunks).unwrap_err(), Error::TooLarge { index: 1 });
    // The same stream, short of the limit, inflates (and fails its checksum).
    let chunks = vec![chunk(0, ChunkKind::Header), Chunk::new(1, ChunkKind::Replay, bomb(10))];
    assert_eq!(Film::from_chunks(chunks).unwrap_err(), Error::Inflate { index: 1 });
}
