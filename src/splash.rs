//! The HUD's splash messages: each managed engine participant (one per
//! player) carries the message on its screen as formatted text, a
//! localization key and its parameters, in
//! `managed-object-networked-splash-message-static-component`.

use crate::bits::bits_at;
use crate::components::Params;
use crate::container::Film;
use crate::payload::{for_each_payload, OpKind, Walker};
use std::collections::BTreeMap;

const PARTICIPANT: u32 = 47;
const STATIC_ROW: u32 = 0;

/// One message shown: the frame, its key, its parameters (a player index for
/// a player parameter, -1 for another kind), and how many players saw it.
#[derive(Clone, Debug, PartialEq)]
pub struct Splash {
    pub frame: i64,
    pub key: u32,
    pub params: Vec<i32>,
    /// Per parameter, the value of a number or a string one: what `%i` and
    /// `%s` stand for. None for the other kinds.
    pub values: Vec<Option<Arg>>,
    pub seen_by: u32,
}

/// A number, or a string's localization key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Arg {
    Number(f64),
    Key(u32),
}

/// A parameter's value as read: a number (`[real:1]`, then 24 bits for a
/// real or 32 for an integer) or a string id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Value {
    Real(u32),
    Int(u32),
    Key(u32),
}

/// A 24-bit real: offset binary over -6000 to 6000.
const REAL_RANGE: f64 = 6000.0;

impl Value {
    fn arg(self) -> Arg {
        match self {
            Value::Real(v) => Arg::Number((v as f64 - (1 << 23) as f64) * REAL_RANGE / (1 << 23) as f64),
            Value::Int(v) => Arg::Number(v as i32 as f64),
            Value::Key(v) => Arg::Key(v),
        }
    }
}

/// The formatted text at `p`: `[set:1]`, a key, `[n:3]`
/// parameters of `[kind:3]`: 1 a player `[none:1]` or 5 bits, 2 a number
/// `[real:1]` and 24 or 32 bits, 3 and above 32 bits.
fn text(c: &[u8], mut p: i64) -> Option<(u32, Vec<i32>, Vec<Option<Value>>)> {
    if bits_at(c, p, 1) == 0 {
        return None;
    }
    let key = bits_at(c, p + 1, 32);
    let n = bits_at(c, p + 33, 3);
    p += 36;
    let mut params = Vec::new();
    let mut values = vec![None; n as usize];
    for value in values.iter_mut() {
        let kind = bits_at(c, p, 3);
        p += 3;
        match kind {
            0 => params.push(-1),
            1 => {
                if bits_at(c, p, 1) == 1 {
                    params.push(-1);
                    p += 1;
                } else {
                    params.push(bits_at(c, p + 1, 5) as i32);
                    p += 6;
                }
            }
            2 => {
                let real = bits_at(c, p, 1) == 1;
                *value =
                    Some(if real { Value::Real(bits_at(c, p + 1, 24)) } else { Value::Int(bits_at(c, p + 1, 32)) });
                p += 1 + if real { 24 } else { 32 };
                params.push(-1);
            }
            _ => {
                // 3: a string id, a localization key. Others aren't known.
                if kind == 3 {
                    *value = Some(Value::Key(bits_at(c, p, 32)));
                }
                p += 32;
                params.push(-1);
            }
        }
    }
    Some((key, params, values))
}

pub fn read(film: &Film, params: Params) -> Vec<Splash> {
    let mut walker = Walker::new(&film.chunks[0], params);
    let frames: Vec<_> = film.frames().collect();
    let at: BTreeMap<usize, usize> = frames.iter().enumerate().map(|(k, f)| (f.index, k)).collect();
    let mut seen: BTreeMap<(i64, u32, Vec<i32>, Vec<Option<Value>>), u32> = BTreeMap::new();
    for_each_payload(film, &mut walker, |i, p| {
        let Some(f) = at.get(&i).map(|&k| &frames[k]) else { return };
        for op in p.ops.iter().filter(|o| o.entity_type == Some(PARTICIPANT)) {
            let OpKind::Update { fields, .. } = &op.kind else { continue };
            for &(_, b) in fields.blocks.iter().filter(|x| x.0 == STATIC_ROW) {
                if let Some((key, params, values)) = text(f.chunk, b) {
                    *seen.entry((i as i64, key, params, values)).or_default() += 1;
                }
            }
        }
    });
    seen.into_iter()
        .map(|((frame, key, params, values), seen_by)| Splash {
            frame,
            key,
            params,
            values: values.iter().map(|v| v.map(Value::arg)).collect(),
            seen_by,
        })
        .collect()
}
