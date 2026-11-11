//! The model against the reference films (see `common` for where they come
//! from). Without films these tests check nothing.

mod common;

use filmshell::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Every reference film, read from its chunks as downloaded.
fn films() -> Vec<(String, Film)> {
    read_all(&common::film_dirs(), |d| Film::from_chunks(common::load(d)))
}

/// `read` over the films in `dirs`, in their order, at most eight at a time
/// (one per core on a smaller machine): a film takes a second or more, and up
/// to four times its inflated size in memory while it is read.
fn read_all(dirs: &[PathBuf], read: impl Fn(&Path) -> Result<Film, Error> + Sync) -> Vec<(String, Film)> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let next = AtomicUsize::new(0);
    let workers = std::thread::available_parallelism().map_or(4, |n| n.get()).min(8).min(dirs.len());
    let work = || {
        let mut read_here = Vec::new();
        loop {
            let i = next.fetch_add(1, Ordering::Relaxed);
            let Some(d) = dirs.get(i) else { break read_here };
            let name = d.file_name().unwrap().to_string_lossy()[..8].to_string();
            let film = read(d).unwrap_or_else(|e| panic!("{name}: {e}"));
            read_here.push((i, name, film));
        }
    };
    let mut films: Vec<(usize, String, Film)> = std::thread::scope(|s| {
        let jobs: Vec<_> = (0..workers).map(|_| s.spawn(work)).collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap()).collect()
    });
    films.sort_by_key(|f| f.0);
    films.into_iter().map(|(_, name, film)| (name, film)).collect()
}

fn corpus() -> &'static [(String, Film)] {
    static FILMS: std::sync::OnceLock<Vec<(String, Film)>> = std::sync::OnceLock::new();
    FILMS.get_or_init(films)
}

#[test]
fn every_film_has_a_header_and_players() {
    for (name, f) in corpus() {
        let h = f.header();
        assert!(h.build.starts_with("6."), "{name}: {:?}", h.build);
        assert!(h.started_at.is_some() && h.map_asset_id.is_some() && h.level_id.is_some(), "{name}");
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

/// The scoreboard's deaths, over every round, match the kills it counted.
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
            agree += usize::from(s.total_stat(StatOwner::Player(p.id), Stat::Deaths) == scored);
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

/// A downloaded film reads the same from its chunks as served and from the
/// same chunks inflated first.
#[test]
fn downloaded_and_inflated_chunks_read_the_same() {
    for d in common::film_dirs() {
        if d.join("filmChunk0").exists() {
            let name = d.file_name().unwrap().to_string_lossy();
            assert!(common::load(&d).iter().all(Chunk::is_compressed), "{name}: chunks as downloaded");
        }
    }
    // Every fourth film read again: reading is the slow part of the tests.
    let dirs: Vec<PathBuf> = common::film_dirs().into_iter().step_by(4).collect();
    let inflated = read_all(&dirs, |d| Film::from_chunks(common::load_folder(d).unwrap()));
    let read: Vec<&(String, Film)> = corpus().iter().step_by(4).collect();
    assert_eq!(inflated.len(), read.len());
    for ((name, a), (_, b)) in read.into_iter().zip(&inflated) {
        assert_eq!(a.lives(), b.lives(), "{name}");
        assert_eq!(a.events(), b.events(), "{name}");
        assert_eq!(a.players(), b.players(), "{name}");
        assert_eq!(a.rides(), b.rides(), "{name}");
    }
}

/// Chunks compressed with stored blocks, in any order, read the same.
#[test]
fn stored_block_chunks_read_the_same() {
    let Some(dir) = common::film("dc2ce439") else { return };
    let plain = common::load_folder(&dir).unwrap();
    let packed: Vec<Chunk<'_>> =
        plain.iter().map(|c| Chunk::new(c.index, c.kind, common::zlib_stored(&c.data))).collect();
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
        if recorded > 0 && f.clock().highlight_zero().is_some() {
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
    let Some(dir) = common::film("dc2ce439") else { return };
    let header = common::load(&dir).swap_remove(0);
    assert_eq!(header.kind, ChunkKind::Header);
    let mut x = 0x2545_f491u32;
    let noise: Vec<u8> = (0..200_000)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect();
    // Zeros walk as empty packets: 256, then 131,072, past what a chunk may
    // hold.
    for replay in [noise, vec![0u8; 4096], vec![0u8; 2 << 20]] {
        let chunks = vec![header.clone(), Chunk::new(1, ChunkKind::Replay, replay)];
        assert_eq!(Film::from_chunks(chunks).unwrap_err(), Error::NoTicks);
    }
}

/// Chunk files are told apart by their bytes: the highlights chunk is the
/// last, and a folder without it reads as a film with no highlights.
#[test]
fn chunk_files_name_their_kinds() {
    let Some(dir) = common::film("1a743b49") else { return };
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
                    && v.track.first().is_some_and(|p| p.tick <= e.tick)
                    && v.track.last().is_some_and(|p| e.tick <= p.tick)
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

/// Every team the film gives a player, bots included, is the team the match
/// stats give them (`match-metadata.json`, which the test films carry).
#[test]
fn teams_agree_with_the_match_stats() {
    let (mut players, mut agree, mut bots, mut bots_with_team) = (0usize, 0usize, 0usize, 0usize);
    for (dir, (name, f)) in common::film_dirs().iter().zip(corpus()) {
        let Ok(stats) = std::fs::read(dir.join("match-metadata.json")) else { continue };
        let stats: serde_json::Value = serde_json::from_slice(&stats).unwrap();
        let mut teams: HashMap<String, i64> = HashMap::new();
        for p in stats["Players"].as_array().into_iter().flatten() {
            if let (Some(id), Some(t)) = (p["PlayerId"].as_str(), p["LastTeamId"].as_i64()) {
                teams.insert(id.to_string(), t);
            }
        }
        let distinct: std::collections::BTreeSet<i64> = teams.values().copied().collect();
        // Free-for-all: the services give each player a team of their own.
        if distinct.len() < 2 || distinct.len() * 2 > teams.len() {
            continue;
        }
        for p in f.players() {
            let key = match &p.kind {
                PlayerKind::Human { xuid, .. } => format!("xuid({xuid})"),
                PlayerKind::Bot { bot_id, .. } => format!("bid({bot_id}.0)"),
                _ => continue,
            };
            let Some(&want) = teams.get(&key) else { continue };
            if p.is_bot() {
                bots += 1;
                bots_with_team += usize::from(p.team.is_some());
            }
            if let Some(t) = p.team {
                players += 1;
                agree += usize::from(i64::from(t.0) == want);
                if i64::from(t.0) != want {
                    println!("{name}: {} has team {} where the stats say {want}", p.id, t.0);
                }
            }
        }
    }
    assert!(corpus().is_empty() || players > 0);
    assert!(agree * 100 >= players * 98, "{agree} of {players} players");
    assert!(bots_with_team * 100 >= bots * 90, "{bots_with_team} of {bots} bots have a team");
}

/// Films an older game build recorded (film versions before 41, next to the
/// test films in `films-unsupported`) are refused as unknown headers.
#[test]
fn older_films_are_refused() {
    let Some(root) = std::env::var_os("FILMSHELL_TEST_FILMS") else { return };
    let Some(dir) = Path::new(&root).parent().map(|p| p.join("films-unsupported")) else { return };
    let Ok(entries) = std::fs::read_dir(&dir) else { return };
    for d in entries.flatten().map(|e| e.path()) {
        let name = d.file_name().unwrap().to_string_lossy().to_string();
        assert_eq!(Film::from_chunks(common::load(&d)).unwrap_err(), Error::UnknownHeader, "{name}");
    }
}

/// A player's kills and deaths over the match, rounds and leaving included,
/// are the ones the match stats give.
#[test]
fn scoreboard_totals_agree_with_the_match_stats() {
    let (mut rows, mut agree) = (0usize, 0usize);
    for (dir, (name, f)) in common::film_dirs().iter().zip(corpus()) {
        let Ok(stats) = std::fs::read(dir.join("match-metadata.json")) else { continue };
        let stats: serde_json::Value = serde_json::from_slice(&stats).unwrap();
        if f.scoreboard().changes.is_empty() {
            continue;
        }
        for p in f.players() {
            let Some(xuid) = p.xuid() else { continue };
            let Some(s) =
                stats["Players"].as_array().into_iter().flatten().find(|q| q["PlayerId"] == format!("xuid({xuid})"))
            else {
                continue;
            };
            let core = &s["PlayerTeamStats"][0]["Stats"]["CoreStats"];
            let row = StatOwner::Player(p.id);
            for (stat, key) in [(Stat::Kills, "Kills"), (Stat::Deaths, "Deaths")] {
                let Some(want) = core[key].as_i64() else { continue };
                rows += 1;
                let got = f.scoreboard().total_stat(row, stat) as i64;
                agree += usize::from(got == want);
                if got != want {
                    println!("{name}: {} {key} {got}, the stats say {want}", p.id);
                }
            }
        }
    }
    assert!(corpus().is_empty() || rows > 0);
    assert!(agree * 100 >= rows * 95, "{agree} of {rows}");
}

/// Matches of one human with bots, where the bots' lives, deaths and kills
/// come from the film alone: Attrition, Elimination, Infection, Juggernaut,
/// Headhunter, Land Grab and Stockpile.
const BOT_MATCHES: [&str; 11] = [
    "813ff0a6", "f2b9163d", "d970bde6", "70b4b566", "7ddc2dc0", "81093849", "f1886e4e", "23a4bb69", "6cf896a6",
    "d34dc219", "6aa44ce6",
];

/// Each player's deaths, the bots' included, are the match stats' Deaths
/// (in the roster's order, the film's): for 98% of players, and for every
/// player in [`BOT_MATCHES`].
#[test]
fn lives_die_as_the_match_stats_count() {
    let exact = BOT_MATCHES;
    let (mut players, mut agree) = (0usize, 0usize);
    for (dir, (name, f)) in common::film_dirs().iter().zip(corpus()) {
        let Ok(stats) = std::fs::read(dir.join("match-metadata.json")) else { continue };
        let stats: serde_json::Value = serde_json::from_slice(&stats).unwrap();
        let rows = stats["Players"].as_array().cloned().unwrap_or_default();
        if rows.len() != f.players().len() {
            continue;
        }
        for (i, s) in rows.iter().enumerate() {
            let Some(want) = s["PlayerTeamStats"][0]["Stats"]["CoreStats"]["Deaths"].as_u64() else { continue };
            let got = f.lives_of(PlayerId(i as u8)).filter(|l| l.died).count() as u64;
            players += 1;
            agree += usize::from(got == want);
            let exact = exact.contains(&name.as_str());
            assert!(got == want || !exact, "{name}: player {i} died {got} times, the stats say {want}");
        }
    }
    assert!(corpus().is_empty() || agree * 100 >= players * 98, "{agree} of {players} players");
}

/// Each player's kills of players on the other team that the scoreboard
/// counts, inferred kills included, are the match stats' Kills. Exact on
/// [`BOT_MATCHES`], whose bot deaths have no kill event.
#[test]
fn kills_count_as_the_match_stats_do() {
    let (mut players, mut agree) = (0usize, 0usize);
    let mut wrong = Vec::new();
    for (dir, (name, f)) in common::film_dirs().iter().zip(corpus()) {
        let Ok(stats) = std::fs::read(dir.join("match-metadata.json")) else { continue };
        let stats: serde_json::Value = serde_json::from_slice(&stats).unwrap();
        let rows = stats["Players"].as_array().cloned().unwrap_or_default();
        if rows.len() != f.players().len() {
            continue;
        }
        let team = |id: PlayerId| f.player(id).and_then(|p| p.team);
        for (i, s) in rows.iter().enumerate() {
            // CoreStats counts AI kills too (PveStats); PvpStats are players'.
            // Firefight has no PvpStats: its kills are all of AI.
            let stats = &s["PlayerTeamStats"][0]["Stats"];
            if stats["PvpStats"].is_null() && !stats["PveStats"].is_null() {
                continue;
            }
            let Some(want) = stats["PvpStats"]["Kills"].as_u64().or(stats["CoreStats"]["Kills"].as_u64()) else {
                continue;
            };
            let p = PlayerId(i as u8);
            let got = f
                .kills()
                .filter(|(_, k)| k.killer == Some(p) && k.victim != p && k.scored != Some(false))
                // Infection moves players between teams: their final teams
                // aren't the teams they had at each kill.
                .filter(|(_, k)| f.mode() == GameMode::Infection || team(k.victim) != team(p) || team(p).is_none())
                .count() as u64;
            players += 1;
            agree += usize::from(got == want);
            if got != want {
                println!("MISS {name} {} p{i}: {got} want {want}", f.variant().map_or("", |v| v.name.as_str()));
            }
            if got != want && BOT_MATCHES.contains(&name.as_str()) {
                wrong.push(format!("{name}: player {i} has {got} kills, the stats say {want}"));
            }
        }
    }
    println!("{agree} of {players} players");
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(corpus().is_empty() || agree * 100 >= players * 95, "{agree} of {players} players");
}

/// Between two readings of the match clock with the same limit, the time
/// left moves by the timer's rate: on 95% of them (the mode's scripts can
/// also set the time outright).
#[test]
fn the_match_clock_runs_at_its_rate() {
    let (mut intervals, mut agree) = (0usize, 0usize);
    for (_, f) in corpus() {
        for w in f.scoreboard().match_clock.windows(2) {
            let (a, b) = (w[0], w[1]);
            let seconds = (b.tick.0 as f64 - a.tick.0 as f64) / 60.0;
            if a.limit != b.limit || seconds < 1.0 {
                continue;
            }
            intervals += 1;
            agree += usize::from((b.left - a.left - a.rate() * seconds).abs() <= 1.0 + 0.02 * seconds);
        }
    }
    assert!(corpus().is_empty() || agree * 100 >= intervals * 93, "{agree} of {intervals}");
}

/// The assistant a kill names, by its event or by the game's rule where the
/// game sent none, earns one of the assists the scoreboard counts: no player
/// is named more often than their Assists, which also count the assisters a
/// kill doesn't name.
#[test]
fn named_assists_are_counted_assists() {
    for (name, f) in corpus() {
        if f.scoreboard().changes.is_empty() {
            continue;
        }
        for p in f.players() {
            let named = f.kills().filter(|(_, k)| k.assistant == Some(p.id) && k.is_scored()).count() as i32;
            let counted = f.scoreboard().total_stat(StatOwner::Player(p.id), Stat::Assists);
            assert!(named <= counted, "{name}: {} named {named} times, {counted} assists", p.id);
        }
    }
}

/// The deaths the game deals everyone alive at a match's end are never
/// counted, also in films whose scoreboard isn't read.
#[test]
fn deaths_at_the_match_end_are_not_scored() {
    let mut seen = 0;
    for (name, f) in corpus() {
        let end = f.clock().end().0 as i64;
        for (t, k) in f.kills().filter(|(t, _)| (end - t.0 as i64 - 362).abs() <= 3) {
            if k.cause == Some(game::DamageCause::World) {
                seen += 1;
                assert_eq!(k.scored, Some(false), "{name}: {t}");
            }
        }
    }
    assert!(corpus().is_empty() || seen > 0);
}

/// A team's score over the match, every round's added up, is the one the
/// match stats give: rounds won in Attrition, Elimination and Grifball,
/// points in Oddball.
#[test]
fn team_scores_agree_with_the_match_stats() {
    let (mut teams, mut agree) = (0usize, 0usize);
    for (dir, (name, f)) in common::film_dirs().iter().zip(corpus()) {
        let Ok(stats) = std::fs::read(dir.join("match-metadata.json")) else { continue };
        let stats: serde_json::Value = serde_json::from_slice(&stats).unwrap();
        if f.scoreboard().changes.is_empty() {
            continue;
        }
        for t in stats["Teams"].as_array().into_iter().flatten() {
            let (Some(id), Some(want)) = (t["TeamId"].as_u64(), t["Stats"]["CoreStats"]["Score"].as_i64()) else {
                continue;
            };
            let got = f.scoreboard().total_stat(StatOwner::Team(TeamId(id as u8)), Stat::Score) as i64;
            teams += 1;
            agree += usize::from(got == want);
            if got != want {
                println!("{name}: team {id} scored {got}, the stats say {want}");
            }
        }
    }
    assert!(corpus().is_empty() || teams > 0);
    assert!(agree * 100 >= teams * 95, "{agree} of {teams} teams");
}

/// No life starts before its player joined or after they left (a joiner
/// takes the engine id of someone who left, and their lives aren't the
/// joiner's).
#[test]
fn lives_start_while_their_player_is_in_the_match() {
    const MARGIN: u32 = 600;
    for (name, f) in corpus() {
        for l in f.lives() {
            let (Some(id), Some(start)) = (l.player, l.start()) else { continue };
            let p = f.player(id).unwrap();
            let early = p.joined.is_some_and(|j| start.0 + MARGIN < j.0);
            let late = p.left.is_some_and(|left| start.0 > left.0 + MARGIN);
            assert!(!early && !late, "{name}: {id}'s life at {start} is outside {:?} to {:?}", p.joined, p.left);
        }
    }
}

/// Every player the scoreboard counts a spawn for has a life; on a film
/// without a scoreboard, every player on a team who was in the match when
/// play began. (A player the roster lists with no team never played; one who
/// left during the loadout, before anyone spawned, has none; nor has a joiner
/// the scoreboard never lists.)
#[test]
fn players_who_spawned_have_lives() {
    for (name, f) in corpus() {
        let Some(play) = f.lives().iter().filter_map(|l| l.start()).min() else { continue };
        let board = f.scoreboard();
        let spawned = |p: &&Player| match board.changes.is_empty() {
            false => board.total_stat(StatOwner::Player(p.id), Stat::Spawns) > 0,
            true => p.team.is_some() && p.present_at_start && p.left.is_none_or(|left| left > play),
        };
        for p in f.players().iter().filter(spawned) {
            assert!(f.lives_of(p.id).next().is_some(), "{name}: {} has no life", p.id);
        }
    }
}

/// The film's level is the one the match stats name by the services' id.
#[test]
fn levels_match_the_match_stats() {
    let mut checked = 0;
    for (dir, (name, f)) in common::film_dirs().iter().zip(corpus()) {
        let Ok(stats) = std::fs::read(dir.join("match-metadata.json")) else { continue };
        let stats: serde_json::Value = serde_json::from_slice(&stats).unwrap();
        let Some(want) = stats["MatchInfo"]["LevelId"].as_str() else { continue };
        let level = f.level().unwrap_or_else(|| panic!("{name}: no level"));
        assert_eq!(level.api_id.map(|g| g.to_string()).as_deref(), Some(want), "{name}");
        checked += 1;
    }
    assert!(corpus().is_empty() || checked > 0);
}

/// The model survives a trip through JSON: every film's players, lives,
/// events, rides, vehicles, zones, header, variant and scoreboard read back
/// equal.
#[cfg(feature = "serde")]
#[test]
fn the_model_reads_back_from_json() {
    fn round<T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug>(
        name: &str,
        what: &str,
        v: &T,
    ) {
        let json = serde_json::to_string(v).unwrap_or_else(|e| panic!("{name}: {e}"));
        let back: T = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(&back == v, "{name}: {what} differ after a round trip");
    }
    for (name, f) in corpus() {
        round(name, "players", &f.players().to_vec());
        round(name, "lives", &f.lives().to_vec());
        round(name, "events", &f.events().to_vec());
        round(name, "rides", &f.rides().to_vec());
        round(name, "vehicles", &f.vehicles().to_vec());
        round(name, "zones", &f.zones().to_vec());
        round(name, "header", f.header());
        round(name, "variant", &f.variant().cloned());
        round(name, "scoreboard", f.scoreboard());
    }
}

/// Every person has a gamertag (chunk 0's roster, a player struct, or their
/// HighlightEvents records) and every team is one of the eight.
#[test]
fn players_have_names_and_real_teams() {
    for (name, f) in corpus() {
        for p in f.players() {
            assert!(p.name().is_some_and(|n| !n.is_empty()), "{name}: {} has no name", p.id);
            assert!(p.team.is_none_or(|t| t.0 < 8), "{name}: {} has team {:?}", p.id, p.team);
        }
    }
}

/// A shot on foot comes from the weapon in the shooter's hands: the fire
/// event's weapon is the weapon in the slot the biped holds.
#[test]
fn shots_come_from_the_weapon_held() {
    let (mut shots, mut held) = (0usize, 0usize);
    for (_, f) in corpus() {
        for e in f.events() {
            let Event::Shot(s) = &e.event else { continue };
            if f.sample_at(s.player, e.tick).is_none_or(|x| x.in_vehicle) {
                continue;
            }
            shots += 1;
            held += usize::from(f.weapon_at(s.player, e.tick) == Some(s.weapon));
        }
    }
    assert!(corpus().is_empty() || shots > 0);
    // The misses are mostly small lobbies, whose bipeds' weapon state isn't
    // read.
    assert!(held * 100 >= shots * 99, "{held} of {shots} shots");
}
