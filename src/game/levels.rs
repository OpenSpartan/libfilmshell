//! Levels and the boxes their positions are quantized over.
//!
//! A film stores a position as an N-bit integer per axis over a box it never
//! carries: the bounds of a structure BSP of the level. With the box a
//! position converts exactly: `min + (q + 0.5) * (max - min) / 2^N` in
//! engine units, and world units are ten engine units.

use super::levels_data::{BUILD, LEVELS};
use crate::TagId;

/// One structure BSP's box, in engine units, with the width of each axis'
/// field.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct LevelBox {
    /// The box's index in a position update.
    pub index: u32,
    /// The low corner.
    pub min: [f32; 3],
    /// The high corner.
    pub max: [f32; 3],
    /// Bits per axis: `ceil(log2(60 * extent))`.
    pub widths: [u32; 3],
}

impl LevelBox {
    /// The engine-unit coordinate of field value `q` on `axis`.
    pub fn engine(&self, axis: usize, q: u32) -> f64 {
        let (min, max) = (self.min[axis] as f64, self.max[axis] as f64);
        min + (q as f64 + 0.5) * (max - min) / 2f64.powi(self.widths[axis] as i32)
    }
}

/// A level: its play space's box and its other structure BSPs.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Level {
    /// The level's `levl` tag id, which the map variant names.
    pub id: TagId,
    /// The level's file name ("ctf_aquarius", "fo11_blank").
    pub name: &'static str,
    /// The play space's structure BSP tag.
    pub structure: TagId,
    /// The play space's box.
    pub play: LevelBox,
    /// Bits of the box index in a position update.
    pub index_bits: u32,
    /// The level's other structure BSPs.
    pub others: &'static [LevelBox],
}

impl Level {
    /// The box with index `index`.
    pub fn level_box(&self, index: u32) -> Option<&LevelBox> {
        if index == self.play.index {
            return Some(&self.play);
        }
        self.others.iter().find(|b| b.index == index)
    }
}

/// The level whose `levl` tag id is `id`, when the catalogue has it.
pub fn level(id: TagId) -> Option<&'static Level> {
    LEVELS.binary_search_by_key(&id, |l| l.id).ok().map(|i| &LEVELS[i])
}

/// Every catalogued level, in tag id order.
pub fn levels() -> &'static [Level] {
    LEVELS
}

/// The game build the level catalogue was read from.
pub fn catalogue_build() -> &'static str {
    BUILD
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The widths follow from the boxes by the precision rule (one step at
    /// most 1/60 engine unit).
    #[test]
    fn widths_follow_from_the_boxes() {
        assert!(LEVELS.windows(2).all(|w| w[0].id < w[1].id));
        for l in LEVELS {
            for b in std::iter::once(&l.play).chain(l.others) {
                for k in 0..3 {
                    let extent = b.max[k] as f64 - b.min[k] as f64;
                    let w = (60.0 * extent).ceil().log2().ceil() as u32;
                    assert_eq!(w, b.widths[k], "{} box {} axis {k}", l.name, b.index);
                }
            }
        }
    }
}
