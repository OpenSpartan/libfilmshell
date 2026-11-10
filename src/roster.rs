//! Who played: the players present at the start (chunk 0's roster table),
//! the humans who joined later (type-8 player structs), and the bots (type-12
//! bot records), each with the frames they were listed in.
//!
//! Players get indices in the order they first appear: everyone present at
//! the start by roster position, then joiners and bots as they are listed.
//! The events and the scoreboard name players by their engine player id,
//! which isn't the index: both listings give each player's id (`Assignment`),
//! and a joiner can take the id of someone who left.

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
/// Bits of a participant handle in a type-8 payload: `[none:1][index:5]`.
const PLAYER_ID_BITS: i64 = 6;
/// A bot record (reader `1429875e4`): `[bot index:32][participant:32]
/// [profile:32]`, then the player struct, with the bot's name in UTF-16
/// big-endian at this byte offset. The bot index is the bot's slot in the
/// game's bot table, the participant its engine player id, and the profile
/// the seed that picks its appearance (the services' bot id).
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

/// A player listed with engine player id `id` from `frame` on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub id: u32,
    pub frame: i64,
    pub player: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Roster {
    pub players: Vec<RosterPlayer>,
    /// Each player's engine player id, from the listing that first gives it
    /// and from each one that changes it, in frame order.
    pub ids: Vec<Assignment>,
}

impl Roster {
    /// Marks the bots whose engine id the first snapshot lists as present
    /// from the start: the first chunk lists no bot records, so theirs come
    /// from the second chunk on. A bot whose id someone held before it isn't.
    pub fn mark_present(&mut self, ids: &[u32]) {
        for (i, p) in self.players.iter_mut().enumerate() {
            let early = p.first_chunk <= 2;
            let Some(a) = self.ids.iter().find(|a| a.player == i as u32) else { continue };
            let first = self.ids.iter().all(|b| b.id != a.id || b.frame >= a.frame || b.player == a.player);
            if p.bot.is_some() && early && first && ids.contains(&a.id) {
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

/// The players in a type-8 payload, (engine player id, XUID), in record
/// order. The payload is `[count:32]`, then per player its participant
/// handle `[none:1][index:5]` and the player struct, which opens with the
/// XUID little-endian and holds it again inside, where fields of 10, 14, 6,
/// 8 and 7 bits follow it that read 183 and zeros (reader `142987bd4`; the
/// handle's reader `1407f2058` is the one the kill events' players use, so
/// the index is the engine player id the events and the scoreboard name). A
/// XUID counts when its inner copy is there; its handle is the 6 bits before
/// its first copy. A record whose handle is none is left out.
fn struct_players(p: &[u8]) -> Vec<(u32, u64)> {
    // Each XUID's first bit and whether its inner copy was seen.
    let mut found: Vec<(u64, i64, bool)> = Vec::new();
    for (s, b) in shifts(p).iter().enumerate() {
        for (i, w) in b.windows(8).enumerate() {
            let x = u64::from_le_bytes(w.try_into().unwrap_or([0; 8]));
            if (x >> 48) as u32 != XUID_HI || x as u32 == 0 {
                continue;
            }
            let at = 8 * i as i64 + s as i64;
            let inner = bits_at(p, at + 64, 10) == INNER_MARK && bits_at(p, at + 74, 32) >> 3 == 0;
            match found.iter_mut().find(|f| f.0 == x) {
                Some(f) => {
                    f.1 = f.1.min(at);
                    f.2 |= inner;
                }
                None => found.push((x, at, inner)),
            }
        }
    }
    found.retain(|f| f.2 && f.1 >= PLAYER_ID_BITS);
    found.sort_by_key(|f| f.1);
    found
        .iter()
        .filter_map(|&(x, at, _)| {
            let handle = bits_at(p, at - PLAYER_ID_BITS, PLAYER_ID_BITS as u32);
            (handle & 0x20 == 0).then_some((handle, x))
        })
        .collect()
}

fn bot_name(p: &[u8], at: i64) -> String {
    let units: Vec<u16> =
        (0..BOT_NAME_UNITS).map(|k| bits_at(p, at + 16 * k, 16) as u16).take_while(|&u| u != 0).collect();
    String::from_utf16_lossy(&units)
}

fn plausible_bot(p: &[u8], at: i64, k: u32) -> bool {
    let printable = |u: u32| (0x20..0x7f).contains(&u);
    bits_at(p, at, 32) == k
        && bits_at(p, at + 32, 32) < 32
        && bits_at(p, at + 64, 32) < 1 << 16
        && printable(bits_at(p, at + 8 * BOT_NAME_BYTE, 16))
}

/// The bots in a type-12 payload: `[count:32]` then one record per bot, the
/// first at bit 32 and each later one somewhere within `BOT_RECORD_BITS`
/// of the previous. Returns (participant, profile, name).
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
    let mut frame = 0i64;
    let mut last8: &[u8] = &[];
    let mut last12: &[u8] = &[];
    // Each bot id's player.
    let mut bots: HashMap<u32, usize> = HashMap::new();
    let mut first_listing = true;
    // Each player's engine id as last listed.
    let mut held: HashMap<usize, u32> = HashMap::new();
    let mut ids = Vec::new();
    let mut assign = |player: usize, id: u32, frame: i64| {
        if held.insert(player, id) != Some(id) {
            ids.push(Assignment { id, frame, player: player as u32 });
        }
    };
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
                let listed = struct_players(body);
                for p in players.iter_mut().filter(|p| p.bot.is_none()) {
                    let on = p.xuid.is_some_and(|x| listed.iter().any(|l| l.1 == x));
                    if on {
                        p.first.get_or_insert(frame);
                        p.last = Some(frame);
                        p.left = None;
                    } else if p.last.is_some() && p.left.is_none() {
                        p.left = Some(frame);
                    }
                }
                for &(id, x) in &listed {
                    let i = match players.iter().position(|p| p.xuid == Some(x)) {
                        Some(i) => i,
                        None => {
                            players.push(RosterPlayer {
                                xuid: Some(x),
                                present_at_start: first_listing,
                                first: Some(frame),
                                last: Some(frame),
                                ..RosterPlayer::default()
                            });
                            players.len() - 1
                        }
                    };
                    assign(i, id, frame);
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
                for (id, bid, name) in listed {
                    let i = match bots.get(&bid) {
                        Some(&i) => i,
                        None => {
                            players.push(RosterPlayer {
                                bot: Some((bid, name)),
                                first: Some(frame),
                                last: Some(frame),
                                first_chunk: ci,
                                ..RosterPlayer::default()
                            });
                            bots.insert(bid, players.len() - 1);
                            players.len() - 1
                        }
                    };
                    assign(i, id, frame);
                }
            }
        }
    }

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
    Roster { players, ids }
}
