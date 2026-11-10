//! An object's creation data, read as the game reads it: each entity type's
//! creation reader (slot `+0x60` of its definition) and the common block
//! they share.
//!
//! In a snapshot the creation data ends on the `[m:32]` that follows it,
//! which checks a reader exactly.

use crate::events::{Bits, Handle};

/// What the creation data names.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Creation {
    /// The object's tag id.
    pub tag: u32,
    /// The variant's string id; None for the tag's default variant.
    pub variant: Option<u32>,
    /// The bit after the creation data.
    pub end: i64,
    /// A projectile's player index and shooter's unit, when it names them.
    pub shooter: (Option<u32>, Option<Handle>),
}

/// `[1]` and an 8-bit version, else `default`.
fn version(r: &mut Bits, default: u32) -> u32 {
    if r.flag() {
        r.bits(8)
    } else {
        default
    }
}

fn opt_tag(r: &mut Bits) {
    if r.flag() {
        r.bits(32);
    }
}

/// `[1]` none, else a 5-bit player index.
fn player(r: &mut Bits) {
    if !r.flag() {
        r.bits(5);
    }
}

/// The block every object opens with.
fn common(r: &mut Bits, c: &mut Creation) -> Option<()> {
    r.bits(9);
    c.tag = r.bits(32);
    if !r.flag() {
        c.variant = Some(r.bits(32));
    }
    if r.flag() {
        r.bits(18);
    }
    if r.flag() {
        r.bits(13);
    }
    r.bits(2);
    r.bits(5);
    let n = r.bits(3);
    if n >= 5 {
        return None;
    }
    for _ in 0..n {
        r.bits(5);
        opt_tag(r);
    }
    if r.flag() {
        player(r);
        if r.flag() {
            r.bits(8);
        }
        r.bits(8);
        r.bits(8);
    }
    if r.flag() {
        r.bits(32);
        opt_tag(r);
        r.bits(14);
    }
    Some(())
}

/// The item reader, which weapons and equipment open with.
fn item(r: &mut Bits, c: &mut Creation) -> Option<()> {
    version(r, 0);
    common(r, c)
}

/// How a position is written: three raw floats inside a snapshot, else the
/// level's quantized codes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Snapshot,
    Tick(crate::events::Layout),
}

fn position(r: &mut Bits, form: Form) {
    match form {
        Form::Snapshot => r.at += 96,
        Form::Tick(widths) => r.pos(widths, 16),
    }
}

/// `[1]` a count (4 bits) of optional tags, else one optional tag.
fn tag_list(r: &mut Bits) {
    if r.flag() {
        for _ in 0..r.bits(4) {
            opt_tag(r);
        }
    } else {
        opt_tag(r);
    }
}

/// The width of a networked property's value of `kind`.
pub fn property_bits(kind: u32) -> u32 {
    match kind {
        1 | 11 => 4,
        2 | 10 => 1,
        3 | 7 => 24,
        _ => 32,
    }
}

/// The bit after a non-object entity's creation data; None for an object
/// type, or one not read yet.
pub fn manager_end(data: &[u8], at: i64, entity_type: u32) -> Option<i64> {
    let mut r = Bits { c: data, at };
    match entity_type {
        // Keyframing.
        3 => {
            version(&mut r, 0);
            r.opt_handle(0);
            if r.flag() {
                r.bits(8);
            }
            r.bits(1 + 8);
        }
        // Engine player: the player index.
        5 => {
            version(&mut r, 0);
            r.bits(6);
        }
        // Statborg: the player index below 48.
        6 => {
            r.bits(6);
        }
        // Managed player.
        9 => {
            version(&mut r, 0);
            r.bits(6 + 6 + 1);
        }
        // Managed object.
        10 => {
            version(&mut r, 0);
            r.opt_handle(0);
        }
        // Objective, navpoint, respawn manager, radial menu.
        11 | 12 | 29 | 49 => {
            version(&mut r, 0);
        }
        // Networked property: its name and one value, or 32.
        13 => {
            version(&mut r, 0);
            r.bits(32);
            let masked = r.flag();
            for _ in 0..if masked { 32 } else { 1 } {
                let kind = r.bits(4);
                if kind != 0 && (kind < 7) != masked {
                    r.at += i64::from(property_bits(kind));
                }
            }
        }
        // Crew, participant.
        14 | 47 => {
            version(&mut r, 0);
            r.bits(5);
        }
        // Music.
        17 => {
            version(&mut r, 0);
            r.bits(7);
        }
        // Flock.
        21 => {
            r.bits(18);
        }
        // No creation data.
        0 | 1 | 2 | 4 | 15 | 18 | 19 | 22 | 25 | 26 | 27 | 34 | 45 => {}
        _ => return None,
    }
    Some(r.at)
}

/// The creation data of an object of `entity_type` at bit `at`. None when
/// the type's reader isn't written yet, or the data is invalid.
pub fn read(data: &[u8], at: i64, entity_type: u32, form: Form) -> Option<Creation> {
    let mut r = Bits { c: data, at };
    let mut c = Creation::default();
    match entity_type {
        // Item, device.
        36 | 43 => item(&mut r, &mut c)?,
        // Rigid bodies: the block and a handle.
        38 | 39 => {
            item(&mut r, &mut c)?;
            r.opt_handle(0);
        }
        // Equipment.
        37 => {
            version(&mut r, 0);
            item(&mut r, &mut c)?;
            player(&mut r);
            if r.flag() {
                r.bits(32);
            }
        }
        // Weapon.
        42 => {
            version(&mut r, 0);
            item(&mut r, &mut c)?;
            r.bits(12);
            r.bits(7);
            tag_list(&mut r);
            player(&mut r);
        }
        // Biped: the block sits after the representation.
        35 => {
            let v = version(&mut r, 13);
            if r.flag() {
                r.bits(32);
            }
            if v > 10 {
                player(&mut r);
            }
            common(&mut r, &mut c)?;
            if r.flag() {
                r.bits(6);
            }
            r.bits(1);
            if r.flag() {
                r.bits(32);
                position(&mut r, form);
                player(&mut r);
            }
            r.bits(19);
            if v > 5 {
                r.bits(1);
            }
            if v >= 12 {
                opt_tag(&mut r);
            }
        }
        // Vehicle.
        40 => {
            item(&mut r, &mut c)?;
            if r.flag() {
                position(&mut r, form);
                // A direction unless none, and a magnitude.
                if !r.flag() {
                    r.bits(19);
                }
                r.bits(8);
            }
            r.bits(19);
            if r.flag() {
                for _ in 0..r.bits(2) {
                    opt_tag(&mut r);
                }
            } else {
                opt_tag(&mut r);
            }
        }
        // Projectile.
        41 => {
            let v = version(&mut r, 1);
            common(&mut r, &mut c)?;
            if r.flag() {
                c.shooter.0 = Some(r.bits(5));
            }
            if r.flag() {
                c.shooter.1 = r.opt_handle(1);
                if !r.flag() {
                    r.bits(2);
                }
            }
            // A kind, then a target by kind.
            if r.flag() {
                // A raw 32-bit value in a snapshot, a handle in a tick (the
                // reader's last argument).
                if let k @ (1 | 2) = r.bits(2) {
                    match form {
                        Form::Snapshot => {
                            r.bits(32);
                        }
                        Form::Tick(_) => {
                            r.handle(k);
                        }
                    }
                    if k == 1 && r.flag() {
                        r.bits(6);
                    }
                }
            }
            r.bits(1);
            if r.flag() {
                r.bits(5);
                r.bits(5);
            }
            r.bits(5);
            if r.flag() {
                position(&mut r, form);
                r.bits(19);
                r.bits(12);
            }
            if r.flag() {
                r.opt_handle(0);
            }
            if r.flag() {
                // A compact direction, or a position.
                if r.flag() {
                    r.bits(2);
                    r.at += 3 * 13;
                    if r.flag() {
                        r.bits(16);
                    }
                } else {
                    position(&mut r, form);
                }
            }
            if v > 2 {
                r.bits(1);
            }
            if r.flag() {
                player(&mut r);
                r.bits(7);
                r.bits(1);
                r.bits(4);
            }
        }
        _ => return None,
    }
    c.end = r.at;
    Some(c)
}
