//! The component registry at the start of chunk 0: for each replicated
//! object archetype, the names of the components its updates can carry.
//!
//! ```text
//! u32 41, u32 27
//! blocks of 64 rows from byte 8, one per entity type; a row is 260 bytes:
//!   char name[256] (NUL-padded, empty for an unused row), u32
//! ```
//!
//! The block index is the entity type an entity-create names (50 types in
//! the game's registry). The leading `41` is not the block count: blocks
//! 41-49 (projectile, weapon, device, then non-object state) follow it in the
//! same layout, so blocks are read past it until one is entirely empty.
//!
//! An object update lists the components it carries as indices into its
//! archetype's block (`replication.rs`), so the indices the scanners key on
//! are looked up here by name. [`Schema::DEFAULT`] holds the usual indices
//! for a chunk 0 without a readable registry.

const START: usize = 8;
const ROW: usize = 260;
const ROWS: usize = 64;

/// Component indices of one archetype.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Archetype {
    /// Rows in use; a listed index at or past it is not a component.
    pub rows: u32,
    pub position: u32,
    pub actor_control: u32,
    pub aim: u32,
    pub command_tick: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Schema {
    pub biped: Archetype,
    pub vehicle: Archetype,
}

impl Schema {
    pub const DEFAULT: Schema = Schema {
        biped: Archetype { rows: 64, position: 0, actor_control: 19, aim: 21, command_tick: 25 },
        vehicle: Archetype { rows: 48, position: 0, actor_control: 19, aim: 21, command_tick: 25 },
    };

    /// The schema in `chunk0`'s registry; [`Schema::DEFAULT`] for any
    /// archetype it does not name.
    pub fn read(chunk0: &[u8]) -> Schema {
        let blocks = blocks(chunk0);
        let find = |marker: &str, fallback: Archetype| -> Archetype {
            blocks
                .iter()
                .find(|b| b.iter().any(|n| n.starts_with(marker)))
                .and_then(|b| archetype(b))
                .unwrap_or(fallback)
        };
        Schema { biped: find("biped-", Self::DEFAULT.biped), vehicle: find("vehicle-", Self::DEFAULT.vehicle) }
    }
}

/// Every block's per-row versions, the `u32` after each row's name, which
/// the game passes to the component's reader.
pub fn versions(chunk0: &[u8]) -> Vec<Vec<u32>> {
    (0..blocks(chunk0).len())
        .map(|k| {
            (0..ROWS)
                .map(|i| {
                    let at = START + k * ROWS * ROW + i * ROW + 256;
                    chunk0.get(at..at + 4).map_or(0, |b| u32::from_le_bytes(b.try_into().unwrap()))
                })
                .collect()
        })
        .collect()
}

/// Every block's row names ("" for an unused row).
pub fn blocks(chunk0: &[u8]) -> Vec<Vec<String>> {
    let le = |o: usize| chunk0.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    let Some(n) = le(0).filter(|&n| n > 0 && n <= 256) else { return Vec::new() };
    (0..64usize)
        .map_while(|k| {
            let at = START + k * ROWS * ROW;
            let block = chunk0.get(at..at + ROWS * ROW)?;
            // Past the header's count, a block belongs to the registry only if
            // it has rows and every row is empty or a component name.
            let name_like = |r: &[u8]| {
                let end = r[..256].iter().position(|&b| b == 0).unwrap_or(256);
                r[..end].iter().all(|&b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
                    && r[end..256].iter().all(|&b| b == 0)
            };
            if k >= n as usize && (block.chunks(ROW).all(|r| r[0] == 0) || !block.chunks(ROW).all(name_like)) {
                return None;
            }
            Some(
                block
                    .chunks(ROW)
                    .map(|row| {
                        let end = row[..256].iter().position(|&b| b == 0).unwrap_or(256);
                        String::from_utf8_lossy(&row[..end]).into_owned()
                    })
                    .collect(),
            )
        })
        .collect()
}

fn archetype(rows: &[String]) -> Option<Archetype> {
    let at = |name: &str| rows.iter().position(|n| n == name).map(|i| i as u32);
    Some(Archetype {
        rows: rows.iter().rposition(|n| !n.is_empty())? as u32 + 1,
        position: at("object-position-dynamic-precision-component")?,
        actor_control: at("unit-actor-control-component")?,
        aim: at("unit-desired-aiming-vector-component")?,
        command_tick: at("unit-command-tick-component")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_reference_film_has_the_default_schema() {
        for dir in crate::testing::film_dirs() {
            let c0 = crate::testing::chunk(&dir, 0).unwrap();
            assert_eq!(Schema::read(&c0), Schema::DEFAULT, "{}", dir.display());
        }
    }
}
