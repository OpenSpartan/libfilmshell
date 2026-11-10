//! The model against the local reference films. `films/` is local only:
//! without it these tests pass without checking anything.

mod common;

use filmshell::*;
use std::collections::HashMap;
use std::path::PathBuf;

/// Every reference film, read.
fn films() -> Vec<(String, Film)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("films");
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut dirs: Vec<PathBuf> =
        entries.flatten().map(|e| e.path()).filter(|p| p.join("filmChunk0_dec").exists()).collect();
    dirs.sort();
    // Read in parallel: a film takes a second or more.
    std::thread::scope(|s| {
        let jobs: Vec<_> = dirs
            .iter()
            .map(|d| {
                s.spawn(move || {
                    let name = d.file_name().unwrap().to_string_lossy()[..8].to_string();
                    let film =
                        Film::from_chunks(common::load_folder(d).unwrap()).unwrap_or_else(|e| panic!("{name}: {e}"));
                    (name, film)
                })
            })
            .collect();
        jobs.into_iter().map(|j| j.join().unwrap()).collect()
    })
}

fn corpus() -> &'static [(String, Film)] {
    static FILMS: std::sync::OnceLock<Vec<(String, Film)>> = std::sync::OnceLock::new();
    FILMS.get_or_init(films)
}

#[test]
fn every_film_has_a_header_and_players() {
    for (name, f) in corpus() {
        let h = f.header();
        assert!(h.build.as_deref().is_some_and(|b| b.starts_with("6.")), "{name}: {:?}", h.build);
        assert!(h.started_at.is_some() && h.map_asset_id.is_some() && h.level.is_some(), "{name}");
        assert!(!f.players().is_empty(), "{name}");
        let mut ids: Vec<PlayerId> = f.players().iter().map(|p| p.id).collect();
        ids.dedup();
        assert_eq!(ids.len(), f.players().len(), "{name}: player ids repeat");
        assert!(f.variant().is_some_and(|v| !v.name.is_empty()), "{name}");
    }
}

#[test]
fn every_known_level_converts_exactly() {
    for (name, f) in corpus() {
        if f.level().is_some() {
            let c = f.coordinates().unwrap_or_else(|| panic!("{name}: no coordinates"));
            assert_eq!(c.source, CoordinateSource::LevelBox, "{name}");
        }
    }
}

#[test]
fn the_clock_and_the_events_run_forward() {
    for (name, f) in corpus() {
        assert!(f.clock().frame_ticks().windows(2).all(|w| w[0] < w[1]), "{name}: ticks");
        assert!(f.events().windows(2).all(|w| w[0].tick <= w[1].tick), "{name}: events");
        for l in f.lives() {
            assert!(l.samples.windows(2).all(|w| w[0].tick <= w[1].tick), "{name}: a life's samples");
        }
        assert!(f.clock().end() > Tick(0), "{name}");
    }
}

#[test]
fn kills_name_players() {
    for (name, f) in corpus() {
        for (t, k) in f.kills() {
            assert!(f.player(k.victim).is_some(), "{name} {t}: victim {}", k.victim);
            if let Some(killer) = k.killer {
                assert!(f.player(killer).is_some(), "{name} {t}: killer {killer}");
            }
        }
    }
}

/// Positions in world units lie within the level's play box, and the
/// samples' heights within its vertical extent, give or take a little.
#[test]
fn positions_land_in_the_level_box() {
    for (name, f) in corpus() {
        let (Some(level), Some(c)) = (f.level(), f.coordinates()) else { continue };
        if c.source != CoordinateSource::LevelBox {
            continue;
        }
        let b = level.play;
        let (x0, x1) = (-10.0 * b.max[1] as f64, -10.0 * b.min[1] as f64);
        let (y0, y1) = (10.0 * b.min[0] as f64, 10.0 * b.max[0] as f64);
        let (z0, z1) = (10.0 * b.min[2] as f64, 10.0 * b.max[2] as f64);
        let (mut n, mut out) = (0usize, 0usize);
        for s in f.lives().iter().flat_map(|l| &l.samples) {
            let Some(p) = s.position else { continue };
            n += 1;
            let inside = p.x >= x0 - 1.0 && p.x <= x1 + 1.0 && p.y >= y0 - 1.0 && p.y <= y1 + 1.0;
            let level_z = p.z.is_none_or(|z| z >= z0 - 1.0 && z <= z1 + 1.0);
            out += usize::from(!(inside && level_z));
        }
        assert!(n > 0, "{name}: no positions");
        assert!(out * 1000 <= n, "{name}: {out} of {n} samples outside the box");
    }
}

/// Each person's deaths that the scoreboard counted match their death
/// records in the HighlightEvents chunk.
#[test]
fn deaths_agree_with_the_highlights() {
    let (mut players, mut agree) = (0usize, 0usize);
    for (_, f) in corpus() {
        if f.scoreboard().changes.is_empty() {
            continue;
        }
        let mut deaths: HashMap<PlayerId, usize> = HashMap::new();
        for (_, k) in f.kills().filter(|(_, k)| k.scored == Some(true)) {
            *deaths.entry(k.victim).or_default() += 1;
        }
        for p in f.players().iter().filter(|p| !p.is_bot()) {
            let recorded =
                f.highlights().iter().filter(|h| h.player == Some(p.id) && h.kind == HighlightKind::Death).count();
            players += 1;
            agree += usize::from(deaths.get(&p.id).copied().unwrap_or(0) == recorded);
        }
    }
    assert!(agree * 100 >= players * 95, "{agree} of {players} players");
}

/// The scoreboard's deaths match the kills it counted. A player who leaves
/// has their row cleared, so the highest value is the count.
#[test]
fn the_scoreboard_counts_the_kill_events() {
    let (mut players, mut agree) = (0usize, 0usize);
    for (_, f) in corpus() {
        let s = f.scoreboard();
        if s.changes.is_empty() {
            continue;
        }
        for p in f.players().iter().filter(|p| !p.is_bot()) {
            let scored = f.kills().filter(|(_, k)| k.victim == p.id && k.scored == Some(true)).count() as i32;
            players += 1;
            let owner = StatOwner::Player(p.id);
            let most =
                s.changes.iter().filter(|c| c.owner == owner && c.slot == Stat::Deaths.slot()).map(|c| c.value).max();
            agree += usize::from(most.unwrap_or(0) == scored);
        }
    }
    assert!(agree * 100 >= players * 95, "{agree} of {players} players");
}

#[test]
fn the_highlight_clock_places_deaths_at_kills() {
    for (name, f) in corpus() {
        let (mut n, mut near) = (0usize, 0usize);
        for h in f.highlights().iter().filter(|h| h.kind == HighlightKind::Death) {
            let (Some(t), Some(p)) = (h.tick, h.player) else { continue };
            n += 1;
            near += usize::from(f.kills().any(|(kt, k)| k.victim == p && kt.0.abs_diff(t.0) <= 30));
        }
        assert!(near * 100 >= n * 90, "{name}: {near} of {n} deaths");
    }
}

#[test]
fn snapshots_list_entities_in_index_order() {
    let Some((_, f)) = corpus().iter().find(|(n, _)| n == "f0eca791") else { return };
    let snaps = f.raw().snapshots();
    assert!(!snaps.is_empty());
    for s in &snaps {
        assert!(s.entities.windows(2).all(|w| w[0].index < w[1].index));
        assert!(s.entities.iter().filter(|e| e.entity_type == 35).all(|e| (512..768).contains(&e.index)));
    }
    let weapons: Vec<_> = snaps[2].entities.iter().filter(|e| e.entity_type == 42).collect();
    assert!(weapons.len() > 20);
    assert!(weapons.iter().all(|e| e.tag.and_then(game::tag_info).is_some()));
}

#[test]
fn compressed_chunks_read_the_same() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("films/dc2ce439-0b30-445c-8185-19cb7a0a075b");
    if !dir.exists() {
        return;
    }
    let plain = common::load_folder(&dir).unwrap();
    let packed: Vec<Chunk> = plain.iter().map(|c| Chunk::new(c.index, c.kind, common::zlib_stored(&c.data))).collect();
    assert!(packed.iter().all(Chunk::is_compressed));
    let a = Film::from_chunks(plain).unwrap();
    let b = Film::from_chunks(packed.into_iter().rev()).unwrap();
    assert_eq!(a.lives(), b.lives());
    assert_eq!(a.events(), b.events());
    assert_eq!(a.players(), b.players());
}

/// A player killed is not on the map three seconds later unless they have
/// spawned again, and is on it at the kill.
#[test]
fn the_dead_leave_the_map() {
    let (mut kills, mut seen) = (0usize, 0usize);
    for (name, f) in corpus() {
        for (t, k) in f.kills() {
            kills += 1;
            seen += usize::from(f.alive_at(k.victim, t));
            let later = Tick(t.0 + 3 * Tick::PER_SECOND);
            let respawned = f.lives_of(k.victim).any(|l| l.start().is_some_and(|s| s > t && s <= later));
            if !respawned {
                assert!(f.sample_at(k.victim, later).is_none(), "{name} {t}: {} still on the map", k.victim);
                assert!(!f.alive_at(k.victim, later), "{name} {t}");
            }
        }
    }
    assert!(seen * 100 >= kills * 90, "{seen} of {kills} victims placed at their kill");
}

/// Every medal of a player in the film is kept, and placed in the event
/// stream whenever the film fixes the highlight clock.
#[test]
fn every_medal_is_kept() {
    for (name, f) in corpus() {
        let recorded =
            f.highlights().iter().filter(|h| h.player.is_some() && matches!(h.kind, HighlightKind::Medal(_))).count();
        assert_eq!(f.medals().count(), recorded, "{name}");
        let placed = f.medals().filter(|m| m.0.is_some()).count();
        let events = f.events().iter().filter(|e| matches!(e.event, Event::Medal(_))).count();
        assert_eq!(events, placed, "{name}");
        if recorded > 0 && f.events().iter().any(|e| matches!(e.event, Event::GameEvent(_))) {
            assert_eq!(placed, recorded, "{name}: medals without a tick");
        }
    }
    if let Some((_, f)) = corpus().iter().find(|(n, _)| n == "c3790060") {
        assert_eq!(f.events().iter().filter(|e| matches!(e.event, Event::Medal(_))).count(), 458);
    }
}

/// Headshot events are headshot kills: nearly all join a kill by the same
/// player, and no kill takes more than one.
#[test]
fn headshots_mark_their_kills() {
    let (mut events, mut marked) = (0usize, 0usize);
    for (name, f) in corpus() {
        let n = f.events().iter().filter(|e| matches!(e.event, Event::Headshot(_))).count();
        let m = f.kills().filter(|(_, k)| k.headshot == Some(true)).count();
        assert!(m <= n, "{name}");
        if n == 0 {
            assert!(f.kills().all(|(_, k)| k.headshot.is_none()), "{name}");
        }
        events += n;
        marked += m;
    }
    assert!(marked * 100 >= events * 90, "{marked} of {events} headshots on a kill");
}

/// A real header with replay chunks that aren't a film's is refused.
#[test]
fn garbage_replay_chunks_are_refused() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("films/dc2ce439-0b30-445c-8185-19cb7a0a075b");
    if !dir.exists() {
        return;
    }
    let header = std::fs::read(dir.join("filmChunk0_dec")).unwrap();
    let mut x = 0x2545_f491u32;
    let noise: Vec<u8> = (0..200_000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect();
    for replay in [noise, vec![0u8; 4096]] {
        let chunks = vec![Chunk::new(0, ChunkKind::Header, &header[..]), Chunk::new(1, ChunkKind::Replay, replay)];
        assert_eq!(Film::from_chunks(chunks).unwrap_err(), Error::NoTicks);
    }
}

/// Chunk files are told apart by their bytes: the highlights chunk is the
/// last, and a folder without it reads as a film with no highlights.
#[test]
fn chunk_files_name_their_kinds() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("films/1a743b49-a67b-4c2b-99e5-fb79e82d3522");
    if !dir.exists() {
        return;
    }
    let mut chunks = common::load_folder(&dir).unwrap();
    chunks.sort_by_key(|c| c.index);
    let last = chunks.len() - 1;
    for c in &chunks {
        let want = match c.index {
            0 => ChunkKind::Header,
            i if i as usize == last => ChunkKind::Highlights,
            _ => ChunkKind::Replay,
        };
        assert_eq!(c.kind, want, "chunk {}", c.index);
    }
    chunks.pop();
    let without = Film::from_chunks(chunks).unwrap();
    assert!(without.highlights().is_empty());
    assert!(!without.players().is_empty() && without.kills().count() > 0);
}

#[test]
fn rides_come_from_the_parent_state() {
    if corpus().is_empty() {
        return;
    }
    let (mut exits, mut inside) = (0, 0);
    for (name, f) in corpus() {
        for r in f.rides() {
            // The parent state names the seat of every ride it records.
            assert!(r.seat.is_some(), "{name}: {r:?}");
            assert!(r.from <= r.to, "{name}: {r:?}");
            let lives: Vec<&Life> = f.lives_of(r.player).filter(|l| f.life_rides(l).any(|x| x == r)).collect();
            assert_eq!(lives.len(), 1, "{name}: {r:?} is in one life");
        }
        for e in f.events() {
            let Event::VehicleExit(x) = &e.event else { continue };
            // An exit from a vehicle whose track runs through it (a part or
            // a mounted turret sends no positions).
            let tracked = f.vehicles().iter().any(|v| {
                v.entity == x.vehicle
                    && v.track.first().is_some_and(|p| p.0 <= e.tick)
                    && v.track.last().is_some_and(|p| e.tick <= p.0)
            });
            if !tracked {
                continue;
            }
            exits += 1;
            // The exit starts the exit animation; the rider stays seated
            // until it ends, so the ride reaches past the exit or nearly.
            inside +=
                f.rides().iter().any(|r| r.player == x.player && r.from <= e.tick && e.tick.0 <= r.to.0 + 90) as usize;
        }
    }
    assert!(exits > 0 && inside == exits, "{inside} of {exits} exits end a ride");
}

/// Every death a player's HighlightEvents records is a life of theirs that
/// ended in one, players who left included (their scoreboard row is cleared).
#[test]
fn lives_end_in_the_recorded_deaths() {
    let (mut players, mut agree) = (0usize, 0usize);
    for (_, f) in corpus() {
        if f.scoreboard().changes.is_empty() {
            continue;
        }
        for p in f.players().iter().filter(|p| !p.is_bot()) {
            let recorded =
                f.highlights().iter().filter(|h| h.player == Some(p.id) && h.kind == HighlightKind::Death).count();
            let died = f.lives_of(p.id).filter(|l| l.died).count();
            players += 1;
            agree += usize::from(died == recorded);
        }
    }
    assert!(agree * 100 >= players * 97, "{agree} of {players} players");
}
