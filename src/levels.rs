//! The level catalogue as the readers use it: each level's boxes in f64
//! engine units.

use std::collections::HashMap;
use std::sync::OnceLock;

/// One level's box in engine units (a world unit is a tenth of one), with
/// the field width of each axis: `ceil(log2(60 * extent))`.
#[derive(Clone, Debug)]
pub struct LevelBox {
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub widths: [u32; 3],
    /// The play space's box index in a position update and its width: the
    /// level's BSP list position, and 1 bit for up to two BSPs, else
    /// `ceil(log2 n)`. Live Fire is 1 in 2 bits: its positions open `0000 01`.
    pub box_index: u32,
    pub box_bits: u32,
    /// The level's other structure BSPs. A position names its box and is
    /// quantized over it (the Forge canvases' outer BSP is box 1, 18 bits an
    /// axis).
    pub others: Vec<OtherBox>,
}

/// Another structure BSP's box: its index in a position update, bounds and
/// field widths.
#[derive(Clone, Debug, PartialEq)]
pub struct OtherBox {
    pub index: u32,
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub widths: [u32; 3],
}

impl LevelBox {
    /// A position in box `bx` as the play space's c1 and c2 window values
    /// (fractional, and outside the box's range when outside the play space),
    /// so the film's calibration places it: through engine units, `min +
    /// (q + 0.5) * extent / 2^N` in its own box, then back over the play
    /// space's. None for an unknown box.
    pub fn in_play_window(&self, bx: u32, q: [u32; 3]) -> Option<[f64; 2]> {
        if bx == self.box_index {
            return Some([q[0] as f64, q[1] as f64]);
        }
        let o = self.others.iter().find(|o| o.index == bx)?;
        Some([0, 1].map(|k| {
            let engine = o.min[k] + (q[k] as f64 + 0.5) * (o.max[k] - o.min[k]) / 2f64.powi(o.widths[k] as i32);
            (engine - self.min[k]) * 2f64.powi(self.widths[k] as i32) / (self.max[k] - self.min[k]) - 0.5
        }))
    }

    /// A position's height in world units, from box `bx`'s Z field. None for
    /// an unknown box.
    pub fn world_z(&self, bx: u32, q: u32) -> Option<f64> {
        let (min, max, width) = if bx == self.box_index {
            (self.min[2], self.max[2], self.widths[2])
        } else {
            let o = self.others.iter().find(|o| o.index == bx)?;
            (o.min[2], o.max[2], o.widths[2])
        };
        Some(10.0 * (min + (q as f64 + 0.5) * (max - min) / 2f64.powi(width as i32)))
    }

    /// The bit of a position update where c1 starts (the `7` of
    /// `read(7 + off)` for a 1-bit box index; one later per extra bit).
    pub fn c1_at(&self) -> i32 {
        6 + self.box_bits as i32
    }
}

/// `c1_at` for an optional level: 7 when the level isn't catalogued.
pub fn c1_at(level: Option<&LevelBox>) -> i32 {
    level.map_or(7, |b| b.c1_at())
}

fn from_game(l: &crate::game::Level) -> LevelBox {
    let f = |v: [f32; 3]| v.map(|x| x as f64);
    LevelBox {
        min: f(l.play.min),
        max: f(l.play.max),
        widths: l.play.widths,
        box_index: l.play.index,
        box_bits: l.index_bits,
        others: l
            .others
            .iter()
            .map(|o| OtherBox { index: o.index, min: f(o.min), max: f(o.max), widths: o.widths })
            .collect(),
    }
}

/// The level whose `levl` tag id is `id`, if the catalogue has it.
pub fn level(id: u32) -> Option<&'static LevelBox> {
    static LEVELS: OnceLock<HashMap<u32, LevelBox>> = OnceLock::new();
    LEVELS.get_or_init(|| crate::game::levels().iter().map(|l| (l.id.0, from_game(l))).collect()).get(&id)
}
