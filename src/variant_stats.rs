//! The game variant's stat definitions and engine settings, from the same
//! Bond data in chunk 0 as `variant.rs`. `BaseVariantData` is read to its
//! end: 1 options, 2 intro and UI, 3 data dependencies, 4 gameplay
//! miscellany, 5 engine miscellany; the field
//! ids are the game's (`i343.NetProtocol.GameOptions`, `.GameStats`).

const REGION_START: usize = 0xce680;
/// Through the roster, where the variant ends even with a long script.
const REGION_LEN: usize = 0x110000;
const ANCHOR: &[u8] = b"Rounds Won\0";
/// The path that closes the header struct (`variant.rs` MODE_PATH).
const MODE_PATH: &[u8] = b"Multiplayer/";

/// One statborg slot's definition, `EngineStatDefinition`. A field at its
/// default isn't written, so these default to 0.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StatDefinition {
    /// m_statIdentifierRawString, "AccuracyPercentage".
    pub name: String,
    /// m_statIdentifier: murmur3 of `name`.
    pub id: u32,
    /// m_baseEntryType (2: a best-of, 3: computed).
    pub base: i64,
    /// m_matchValueAggregationType, how a match's value comes from its rounds.
    pub aggregation: i64,
    /// m_sortDirection: -1 lower is better.
    pub sort: i64,
    /// m_displayType: 0 a number, 1 a time, 2 a percentage.
    pub display: i64,
    /// m_storedValueDecimalPrecision: the stored value is 10^n times the shown.
    pub precision: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Definitions {
    /// By statborg slot.
    pub stats: Vec<StatDefinition>,
    /// The variant's scoreboard columns (m_scoreboardStatEntries): slots.
    pub scoreboard: Vec<usize>,
    /// m_limitServerFramerateTo30FPS: the film runs at 30 Hz.
    pub server_30fps: bool,
    /// m_tickRate, when the variant writes it.
    pub tick_rate: Option<i64>,
    /// PlaybackSettings, each when written: killcamEnabled,
    /// timeWindowForKillCamSeconds, playOfTheGameEnabled, machinimaControlsEnabled.
    pub kill_cam: Option<bool>,
    pub kill_cam_seconds: Option<i64>,
    pub play_of_the_game: Option<bool>,
    pub machinima: Option<bool>,
    /// RespawnOptions.m_overrideSpawnSettingsTag: an `ssdf` tag id, when the
    /// variant overrides the global default.
    pub spawn_settings: Option<u32>,
}

/// A value of the variant's Bond tree.
enum Node {
    Num(f64),
    Text(Vec<u8>),
    Struct(Vec<(u32, Node)>),
    List(Vec<Node>),
    Other,
}

impl Node {
    fn num(&self) -> Option<f64> {
        match self {
            Node::Num(v) => Some(*v),
            _ => None,
        }
    }
    fn get(&self, id: u32) -> Option<&Node> {
        match self {
            Node::Struct(f) => f.iter().find(|x| x.0 == id).map(|x| &x.1),
            _ => None,
        }
    }
    fn list(&self) -> &[Node] {
        match self {
            Node::List(l) => l,
            _ => &[],
        }
    }
    fn at(&self, path: &[u32]) -> Option<&Node> {
        path.iter().try_fold(self, |n, &id| n.get(id))
    }
    fn int(&self, id: u32) -> Option<i64> {
        self.get(id).and_then(Node::num).map(|v| v as i64)
    }
}

fn varint(b: &[u8], p: &mut usize) -> Option<u64> {
    let (mut v, mut s) = (0u64, 0);
    loop {
        let x = *b.get(*p)?;
        *p += 1;
        v |= ((x & 127) as u64) << s;
        s += 7;
        if x < 128 {
            return Some(v);
        }
        if s > 63 {
            return None;
        }
    }
}

/// How deep values may nest: real variants nest a few levels.
const MAX_DEPTH: u32 = 32;

/// Moves `p` on by `n` bytes, which must lie within `b`.
fn advance(b: &[u8], p: &mut usize, n: u64) -> Option<()> {
    *p = p.checked_add(usize::try_from(n).ok()?).filter(|&e| e <= b.len())?;
    Some(())
}

/// A Bond Compact Binary v2 value of type `t`, `depth` levels down.
fn value(b: &[u8], p: &mut usize, t: u8, depth: u32) -> Option<Node> {
    if depth > MAX_DEPTH {
        return None;
    }
    let zig = |v: u64| (v >> 1) as i64 ^ -((v & 1) as i64);
    Some(match t {
        2 | 3 | 14 => {
            let v = *b.get(*p)? as i8 as f64;
            *p += 1;
            Node::Num(if t == 3 { v.rem_euclid(256.0) } else { v })
        }
        4..=6 => Node::Num(varint(b, p)? as f64),
        15..=17 => Node::Num(zig(varint(b, p)?) as f64),
        7 => {
            let v = f32::from_le_bytes(b.get(*p..*p + 4)?.try_into().ok()?);
            *p += 4;
            Node::Num(v as f64)
        }
        8 => {
            advance(b, p, 8)?;
            Node::Other
        }
        9 => {
            let n = varint(b, p)?;
            let start = *p;
            advance(b, p, n)?;
            Node::Text(b[start..*p].to_vec())
        }
        18 => {
            let n = varint(b, p)?;
            advance(b, p, n.checked_mul(2)?)?;
            Node::Other
        }
        10 => {
            let n = varint(b, p)?;
            let mut end = *p;
            advance(b, &mut end, n)?;
            let fields = fields(b, p, Some(end), depth + 1)?;
            *p = end;
            fields
        }
        11 | 12 => {
            let x = *b.get(*p)?;
            *p += 1;
            let n = if x >> 5 == 0 { varint(b, p)? } else { (x >> 5) as u64 - 1 };
            if x & 31 == 3 {
                // A byte list: the script and the string blob.
                advance(b, p, n)?;
                return Some(Node::Other);
            }
            // Every value takes a byte at least, so the bytes left bound the
            // list.
            let mut out = Vec::with_capacity((n as usize).min(1024));
            for _ in 0..n {
                out.push(value(b, p, x & 31, depth + 1)?);
            }
            Node::List(out)
        }
        13 => {
            let (kt, vt) = (*b.get(*p)?, *b.get(*p + 1)?);
            *p += 2;
            for _ in 0..varint(b, p)? {
                value(b, p, kt, depth + 1)?;
                value(b, p, vt, depth + 1)?;
            }
            Node::Other
        }
        _ => return None,
    })
}

/// A struct's fields, up to its stop or `end`.
fn fields(b: &[u8], p: &mut usize, end: Option<usize>, depth: u32) -> Option<Node> {
    let mut out = Vec::new();
    loop {
        if end.is_some_and(|e| *p >= e) {
            return Some(Node::Struct(out));
        }
        let h = *b.get(*p)?;
        *p += 1;
        let (t, mut id) = (h & 31, (h >> 5) as u32);
        if id == 6 {
            id = *b.get(*p)? as u32;
            *p += 1;
        } else if id == 7 {
            id = *b.get(*p)? as u32 | (*b.get(*p + 1)? as u32) << 8;
            *p += 2;
        }
        match t {
            0 => return Some(Node::Struct(out)),
            1 => continue,
            _ => out.push((id, value(b, p, t, depth)?)),
        }
    }
}

fn shifted(r: &[u8], s: u32) -> Vec<u8> {
    let at = |i: usize| r.get(i).copied().unwrap_or(0) as u32;
    (0..r.len()).map(|i| (((at(i) << 8 | at(i + 1)) << s) >> 8) as u8).collect()
}

/// `BaseVariantData` after its metadata, parsed to its end.
fn base_variant(chunk0: &[u8]) -> Option<Node> {
    let end = chunk0.len().min(REGION_START + REGION_LEN);
    let r = chunk0.get(REGION_START..end)?;
    let b = (0..8).map(|s| shifted(r, s)).find(|b| b.windows(ANCHOR.len()).any(|w| w == ANCHOR))?;
    let at = b.windows(MODE_PATH.len()).position(|w| w == MODE_PATH)?;
    let mut p = at + b[at..].iter().position(|&c| c == 0)? + 1;
    fields(&b, &mut p, None, 0)
}

pub fn read(chunk0: &[u8]) -> Option<Definitions> {
    let v = base_variant(chunk0)?;
    let stats: Vec<StatDefinition> = v
        .at(&[3, 21, 0])?
        .list()
        .iter()
        .map(|s| StatDefinition {
            name: match s.get(15) {
                Some(Node::Text(t)) => String::from_utf8_lossy(t).into_owned(),
                _ => String::new(),
            },
            id: s.int(0).unwrap_or(0) as u32,
            base: s.int(1).unwrap_or(0),
            aggregation: s.int(3).unwrap_or(0),
            sort: s.int(12).unwrap_or(0),
            display: s.int(13).unwrap_or(0),
            precision: s.int(17).unwrap_or(0),
        })
        .collect();
    let scoreboard = v
        .at(&[2, 30])
        .map(|l| {
            l.list().iter().filter_map(|e| stats.iter().position(|s| s.id == e.int(0).unwrap_or(0) as u32)).collect()
        })
        .unwrap_or_default();
    let engine = v.get(5);
    let playback = v.at(&[1, 0, 5]);
    let flag = |id: u32| playback.and_then(|p| p.int(id)).map(|x| x != 0);
    Some(Definitions {
        stats,
        scoreboard,
        server_30fps: engine.and_then(|e| e.int(0)).is_some_and(|x| x != 0),
        tick_rate: engine.and_then(|e| e.int(7)),
        kill_cam: flag(0),
        kill_cam_seconds: playback.and_then(|p| p.int(1)),
        play_of_the_game: flag(2),
        machinima: flag(3),
        spawn_settings: v.at(&[1, 2, 18]).and_then(|s| s.int(0)).map(|t| t as u32),
    })
}
