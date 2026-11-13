//! A look at a film's raw layers: its encoding, its packets by kind, the
//! event types of its tick packets, how many creates, updates and deletes
//! its entity views hold in all, and its seek snapshots.
//!
//! ```text
//! cargo run --release --example raw -- path/to/film
//! ```

mod common;

use filmshell::Film;
use filmshell::format::{self, EntityOp};
use std::collections::BTreeMap;

fn main() -> std::process::ExitCode {
    common::main(run)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args().nth(1).ok_or("usage: raw <film folder>")?;
    let film = Film::from_folder(&dir)?;
    let raw = film.raw();
    println!("encoding {:?}, header chunk {} bytes", raw.encoding(), raw.header_chunk().len());

    let mut kinds: BTreeMap<u16, (usize, usize)> = BTreeMap::new();
    for p in raw.packets() {
        let k = kinds.entry(p.kind).or_default();
        k.0 += 1;
        k.1 += p.payload.len();
    }
    for (kind, (n, bytes)) in &kinds {
        println!("packet kind {kind:>2}: {n:>6} packets, {bytes:>9} bytes");
    }

    let mut events: BTreeMap<u32, usize> = BTreeMap::new();
    for frame in raw.frames() {
        for e in raw.events(&frame) {
            *events.entry(e.kind).or_default() += 1;
        }
    }
    let mut by_count: Vec<_> = events.into_iter().collect();
    by_count.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    println!("event types, most frequent first:");
    for (kind, n) in by_count.iter().take(12) {
        println!("  {kind:>3} {:<32} {n:>7}", format::event_name(*kind).unwrap_or("?"));
    }

    let (mut creates, mut updates, mut deletes) = (0usize, 0usize, 0usize);
    raw.for_each_entity_view(|_, entries| {
        for e in entries {
            match e.op {
                EntityOp::Create { .. } => creates += 1,
                EntityOp::Update { .. } => updates += 1,
                EntityOp::Delete => deletes += 1,
                _ => {}
            }
        }
    });
    println!("entity view: {creates} creates, {updates} updates, {deletes} deletes");
    println!("{} seek snapshots", raw.snapshots().len());
    Ok(())
}
