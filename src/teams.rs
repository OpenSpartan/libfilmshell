//! Each participant's team, from the managed players.
//!
//! **Code.** Every participant (an engine player id, a person's or a bot's)
//! has a managed player entity (type 9). Its creation data names the
//! participant, `[version][participant:6][6][1]` (reader `1410d7540`), and
//! its component row 0, `managed-player-team-designator-component`, is 4
//! bits: the team plus one, 0 for none (gather `142edbd3c`).
//!
//! **Measured.** On the reference films' team games, the designator less one
//! is the team the match stats give 628 of 634 players. In free-for-all
//! games every designator is 1, and the per-player teams the services list
//! aren't in the film.

use crate::bits::bits_at;
use crate::container::Film;
use crate::payload::{MANAGED_PLAYER, OpKind, Payload};
use crate::roster::Roster;
use crate::snapshot;
use std::collections::HashMap;

/// The designators read, keyed as they come in.
#[derive(Debug, Default)]
pub struct Designators {
    /// Each managed player entity's participant.
    participant: HashMap<(u32, u32), u32>,
    /// Each entity's designators: (frame, value).
    values: HashMap<(u32, u32), Vec<(i64, u32)>>,
}

/// Reads the managed players' creates and team designators.
pub fn reader(d: &mut Designators) -> impl FnMut(usize, &[u8], &Payload) + '_ {
    move |frame, c, p| {
        for op in p.ops.iter().filter(|op| op.entity_type == Some(MANAGED_PLAYER)) {
            let key = (op.index, op.generation);
            let fields = match &op.kind {
                OpKind::Create { .. } => {
                    if let Some(id) = op.participant {
                        d.participant.insert(key, id);
                    }
                    op.init.as_ref()
                }
                OpKind::Update { fields, .. } => Some(fields),
                OpKind::Delete => None,
            };
            let Some(f) = fields else { continue };
            for &(_, bit) in f.blocks.iter().filter(|b| b.0 == 0) {
                let v = bits_at(c, bit, 4);
                let series = d.values.entry(key).or_default();
                if series.last().is_none_or(|l| l.1 != v) {
                    series.push((frame as i64, v));
                }
            }
        }
    }
}

/// Each participant's designators over the film, (frame, designator), with
/// the managed players that existed before the recording mapped from the
/// seek snapshots.
pub fn finish(film: &Film, mut d: Designators) -> HashMap<u32, Vec<(i64, u32)>> {
    for ci in 1..film.chunks.len() {
        let Some(ps) = film.packets(ci) else { continue };
        let c = &film.chunks[ci];
        for pk in ps.iter().filter(|p| p.kind == snapshot::ENTITY_TABLE) {
            for e in snapshot::entity_table(c, pk.start, pk.end) {
                if e.entity_type != MANAGED_PLAYER {
                    continue;
                }
                if let Some(id) = crate::creation::managed_player_participant(c, e.creation()) {
                    d.participant.entry((e.index, e.generation)).or_insert(id);
                }
            }
        }
    }
    let mut out: HashMap<u32, Vec<(i64, u32)>> = HashMap::new();
    for (key, series) in d.values {
        if let Some(&id) = d.participant.get(&key) {
            out.entry(id).or_default().extend(series);
        }
    }
    for v in out.values_mut() {
        v.sort_unstable();
    }
    out
}

/// Gives each player without a team the team their participant's designator
/// names while they hold it: the last value up to the end of their tenure,
/// else the first after it began. Nothing on a film whose designators name
/// fewer than two teams.
pub fn assign(roster: &mut Roster, designators: &HashMap<u32, Vec<(i64, u32)>>) {
    let teams: std::collections::BTreeSet<u32> =
        designators.values().flatten().map(|&(_, v)| v).filter(|&v| v > 0).collect();
    if teams.len() < 2 {
        return;
    }
    for i in 0..roster.players.len() {
        if roster.players[i].team.is_some() {
            continue;
        }
        let last = roster.players[i].last.unwrap_or(i64::MAX);
        let mut team = None;
        for a in roster.ids.iter().filter(|a| a.player == i as u32) {
            let Some(series) = designators.get(&a.id) else { continue };
            // The tenure ends where another player takes the id.
            let until = roster
                .ids
                .iter()
                .filter(|b| b.id == a.id && b.frame > a.frame && b.player != a.player)
                .map(|b| b.frame)
                .min()
                .unwrap_or(i64::MAX)
                .min(last);
            let value = series
                .iter()
                .rfind(|&&(f, _)| f <= until && f >= a.frame)
                .or_else(|| series.iter().rfind(|&&(f, _)| f <= a.frame))
                .or_else(|| series.iter().find(|&&(f, _)| f >= a.frame));
            if let Some(&(_, v)) = value {
                team = v.checked_sub(1);
            }
        }
        roster.players[i].team = team;
    }
}
