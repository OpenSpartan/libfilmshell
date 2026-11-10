//! Who played: the players present at the start (chunk 0's roster table),
//! the humans who joined later (type-8 player structs), and the bots (type-12
//! bot records), each with the frames they were listed in.
//!
//! Players get indices in the order they first appear: everyone present at
//! the start by roster position, then joiners and bots as they are listed.
//! A joiner who replaces a bot takes over the bot's engine player id; the
//! takeover is recorded (`Takeover`) so that what the film says about that
//! id afterwards is theirs.

use crate::bits::bits_at;
use crate::container::Film;
use crate::highlights::HighlightEvent;
use crate::identity::extract_roster;
use std::collections::HashMap;

const PLAYER_STRUCTS: u16 = 8;
const BOT_RECORDS: u16 = 12;
/// Every XUID's high 16 bits.
const XUID_HI: u32 = 0x0009;
/// The 10-bit field after a player struct's inner XUID.
const INNER_MARK: u32 = 183;
/// A bot and the joiner who replaced it left and joined this many ticks apart
/// at most.
const TAKEOVER_TICKS: i64 = 120;
/// A bot record: `[index:32][id:32][bot id:32]`, then its name in
/// UTF-16 big-endian at this byte offset.
const BOT_NAME_BYTE: i64 = 120;
const BOT_NAME_UNITS: i64 = 16;
/// The bits a bot record takes at least, and at most: the next record is
/// searched for in between.
const BOT_RECORD_BITS: (i64, i64) = (8 * 1900, 8 * 2300);

/// One player as the film lists them.
#[derive(Clone, Debug, Default)]
pub struct RosterPlayer {
    pub xuid: Option<u64>,
    pub gamertag: Option<String>,
    /// A bot's id and name.
    pub bot: Option<(u32, String)>,
    pub present_at_start: bool,
    /// Frames of the first and the last packet that listed the player.
    pub first: Option<i64>,
    pub last: Option<i64>,
    /// The frame of the first listing without the player after their last.
    pub left: Option<i64>,
    /// The team, from the player's HighlightEvents records.
    pub team: Option<u32>,
    /// The replay chunk of the first listing.
    pub first_chunk: usize,
}

/// A player taking over an engine player id from `frame` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Takeover {
    pub id: u32,
    pub frame: i64,
    pub player: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Roster {
    pub players: Vec<RosterPlayer>,
    pub takeovers: Vec<Takeover>,
}

impl Roster {
    /// Marks the bots whose engine id the first snapshot lists as present
    /// from the start: the first chunk lists no bot records, so theirs come
    /// from the second chunk on.
    pub fn mark_present(&mut self, ids: &[u32]) {
        for (i, p) in self.players.iter_mut().enumerate() {
            let early = p.first_chunk <= 2;
            if p.bot.is_some()
                && early
                && ids.contains(&(i as u32))
                && !self.takeovers.iter().any(|t| t.player == i as u32)
            {
                p.present_at_start = true;
            }
        }
    }
}

/// `p` read from each bit shift: `shifted[s][i]` is the byte that starts `s`
/// bits into byte `i`.
fn shifts(p: &[u8]) -> [Vec<u8>; 8] {
    std::array::from_fn(|s| {
        (0..p.len())
            .map(|i| {
                let hi = (p[i] as u16) << s;
                let lo = p.get(i + 1).map_or(0, |&n| (n as u16) >> (8 - s));
                (if s == 0 { p[i] as u16 } else { hi | lo }) as u8
            })
            .collect()
    })
}

/// The XUIDs in a type-8 payload. Each player struct holds its XUID twice,
/// little-endian: in its prefix, and inside the record, where fields of 10,
/// 14, 6, 8 and 7 bits follow it that read 183 and zeros. The inner copy is
/// the one taken.
fn struct_xuids(p: &[u8]) -> Vec<u64> {
    let mut found: Vec<u64> = Vec::new();
    for (s, b) in shifts(p).iter().enumerate() {
        for (i, w) in b.windows(8).enumerate() {
            let x = u64::from_le_bytes(w.try_into().unwrap_or([0; 8]));
            if (x >> 48) as u32 != XUID_HI || x as u32 == 0 {
                continue;
            }
            let after = 8 * i as i64 + s as i64 + 64;
            if bits_at(p, after, 10) == INNER_MARK && bits_at(p, after + 10, 32) >> 3 == 0 && !found.contains(&x) {
                found.push(x);
            }
        }
    }
    found
}

fn bot_name(p: &[u8], at: i64) -> String {
    let units: Vec<u16> =
        (0..BOT_NAME_UNITS).map(|k| bits_at(p, at + 16 * k, 16) as u16).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

fn plausible_bot(p: &[u8], at: i64, k: u32) -> bool {
    let printable = |u: u32| (0x20..0x7f).contains(&u);
    bits_at(p, at, 32) == k
        && bits_at(p, at + 32, 32) < 64
        && bits_at(p, at + 64, 32) < 1 << 16
        && printable(bits_at(p, at + 8 * BOT_NAME_BYTE, 16))
}

/// The bots in a type-12 payload: `[count:32]` then one record per bot, the
/// first at bit 32 and each later one somewhere within `BOT_RECORD_BITS`
/// of the previous. Returns (id, bot id, name).
fn bot_records(p: &[u8]) -> Vec<(u32, u32, String)> {
    if p.len() < 4 {
        return Vec::new();
    }
    let n = bits_at(p, 0, 32);
    let bits = p.len() as i64 * 8;
    let mut out = Vec::new();
    let mut at = 32;
    for k in 0..n.min(64) {
        if k > 0 {
            let (lo, hi) = (at + BOT_RECORD_BITS.0, (at + BOT_RECORD_BITS.1).min(bits - 96));
            match (lo..hi).find(|&b| plausible_bot(p, b, k)) {
                Some(b) => at = b,
                None => break,
            }
        } else if !plausible_bot(p, at, 0) {
            break;
        }
        out.push((bits_at(p, at + 32, 32), bits_at(p, at + 64, 32), bot_name(p, at + 8 * BOT_NAME_BYTE)));
    }
    out
}

pub fn read(film: &Film, highlights: &[HighlightEvent]) -> Roster {
    let mut players: Vec<RosterPlayer> = extract_roster(&film.chunks[0])
        .into_iter()
        .map(|r| RosterPlayer {
            xuid: r.xuid.parse().ok(),
            gamertag: Some(r.gamertag),
            present_at_start: true,
            ..RosterPlayer::default()
        })
        .collect();
    let mut takeovers = Vec::new();
    let mut frame = 0i64;
    let mut last8: &[u8] = &[];
    let mut last12: &[u8] = &[];
    // Each bot id's player.
    let mut bots: HashMap<u32, usize> = HashMap::new();
    let ticks = film.frame_ticks();
    let tick = |f: i64| ticks.get(f.max(0) as usize).copied().unwrap_or(0) as i64;
    let mut first_listing = true;
    let mut joiners: Vec<usize> = Vec::new();
    for (ci, c) in film.chunks.iter().enumerate().skip(1) {
        let Some(ps) = film.packets(ci) else { continue };
        for pk in ps {
            if pk.kind == 0 {
                frame += 1;
                continue;
            }
            let body = &c[pk.start..pk.end];
            if pk.kind == PLAYER_STRUCTS && body != last8 {
                last8 = body;
                let listed = struct_xuids(body);
                for p in players.iter_mut().filter(|p| p.bot.is_none()) {
                    let on = p.xuid.is_some_and(|x| listed.contains(&x));
                    if on {
                        p.first.get_or_insert(frame);
                        p.last = Some(frame);
                        p.left = None;
                    } else if p.last.is_some() && p.left.is_none() {
                        p.left = Some(frame);
                    }
                }
                for x in listed {
                    if players.iter().any(|p| p.xuid == Some(x)) {
                        continue;
                    }
                    players.push(RosterPlayer {
                        xuid: Some(x),
                        present_at_start: first_listing,
                        first: Some(frame),
                        last: Some(frame),
                        ..RosterPlayer::default()
                    });
                    joiners.push(players.len() - 1);
                }
                first_listing = false;
            }
            if pk.kind == BOT_RECORDS && body != last12 {
                last12 = body;
                let listed = bot_records(body);
                for (&bid, &i) in &bots {
                    let p = &mut players[i];
                    if listed.iter().any(|b| b.1 == bid) {
                        p.last = Some(frame);
                        p.left = None;
                    } else if p.left.is_none() {
                        p.left = Some(frame);
                    }
                }
                for (_, bid, name) in listed {
                    if bots.contains_key(&bid) {
                        continue;
                    }
                    players.push(RosterPlayer {
                        bot: Some((bid, name)),
                        first: Some(frame),
                        last: Some(frame),
                        first_chunk: ci,
                        ..RosterPlayer::default()
                    });
                    bots.insert(bid, players.len() - 1);
                }
            }
        }
    }
    // A joiner who replaces a bot that left about when they joined takes the
    // bot's id: the bot's records can outlast the join by a few frames.
    for &j in &joiners {
        let Some(joined) = players[j].first else { continue };
        let replaced = players
            .iter()
            .enumerate()
            .find(|(_, b)| b.bot.is_some() && b.left.is_some_and(|l| (tick(l) - tick(joined)).abs() <= TAKEOVER_TICKS));
        let Some((b, _)) = replaced else { continue };
        // The bot may itself hold an id it took over.
        let id = takeovers.iter().find(|t: &&Takeover| t.player == b as u32).map_or(b as u32, |t| t.id);
        takeovers.push(Takeover { id, frame: joined, player: j as u32 });
    }
    takeovers.sort_by_key(|t| t.frame);

    // Teams from the players' HighlightEvents records.
    for p in &mut players {
        let Some(x) = p.xuid else { continue };
        let x = x.to_string();
        let mine = highlights.iter().filter(|h| h.xuid == x);
        let mut votes: HashMap<u32, usize> = HashMap::new();
        for h in mine {
            *votes.entry(h.team).or_default() += 1;
            if p.gamertag.is_none() && !h.gamertag.is_empty() {
                p.gamertag = Some(h.gamertag.clone());
            }
        }
        p.team = votes.into_iter().max_by_key(|&(t, n)| (n, std::cmp::Reverse(t))).map(|(t, _)| t);
    }
    Roster { players, takeovers }
}
