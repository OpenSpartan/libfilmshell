//! Map variants against film-assets: its map variants and the objective
//! placements its `tools/objectives` writes from them with this library.
//!
//! film-assets is a private repository. `FILM_ASSETS_DIR` names a checkout;
//! the tests read its map variants (`sources/maps/<asset>/`: the variant's one
//! `.mvar` file and its `version.json`) and its objective placements
//! (`objectives/<asset>.json`). Without the variable they check nothing.
//!
//! The placements are rounded: positions to hundredths, volumes to tenths, a
//! box's heading to thousandths. A change to `MapVariant::objectives` that
//! film-assets hasn't regenerated its placements for fails here.

use filmshell::ZoneArea;
use filmshell::map_variant::{MapVariant, ObjectiveKind, ObjectiveMode};
use serde_json::Value;
use std::path::{Path, PathBuf};

const VAR: &str = "FILM_ASSETS_DIR";

struct Download {
    asset: String,
    version: String,
    map: MapVariant,
    bytes: Vec<u8>,
}

fn root() -> Option<PathBuf> {
    std::env::var_os(VAR).map(PathBuf::from)
}

fn json(path: &Path) -> Value {
    let text = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    serde_json::from_slice(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Every map variant, by asset id.
fn downloads(root: &Path) -> Vec<Download> {
    let maps = root.join("sources").join("maps");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&maps)
        .unwrap_or_else(|e| panic!("{}: {e}", maps.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "{VAR} is set, but {} holds no map variants", maps.display());
    dirs.iter()
        .map(|d| {
            let meta = json(&d.join("version.json"));
            let mvars: Vec<PathBuf> = std::fs::read_dir(d)
                .expect("a map variant's folder")
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "mvar"))
                .collect();
            let [mvar] = &mvars[..] else { panic!("{}: not exactly one .mvar file", d.display()) };
            let bytes = std::fs::read(mvar).expect("a map variant");
            let map = MapVariant::from_bytes(&bytes).unwrap_or_else(|e| panic!("{}: {e}", mvar.display()));
            Download {
                asset: d.file_name().unwrap().to_string_lossy().into_owned(),
                version: meta["VersionId"].as_str().unwrap_or_default().to_owned(),
                map,
                bytes,
            }
        })
        .collect()
}

fn near(a: f64, b: &Value, within: f64) -> bool {
    b.as_f64().is_some_and(|b| (a - b).abs() <= within)
}

/// The asset files' name for a mode.
fn mode_name(m: ObjectiveMode) -> &'static str {
    match m {
        ObjectiveMode::CaptureTheFlag => "ctf",
        ObjectiveMode::KingOfTheHill => "koth",
        ObjectiveMode::Strongholds => "strongholds",
        ObjectiveMode::LandGrab => "landgrab",
        ObjectiveMode::TotalControl => "totalcontrol",
        ObjectiveMode::Extraction => "extraction",
        ObjectiveMode::Oddball => "oddball",
        ObjectiveMode::Assault => "assault",
        _ => "?",
    }
}

/// The asset files' name for a kind.
fn kind_name(k: ObjectiveKind) -> &'static str {
    match k {
        ObjectiveKind::FlagSpawn => "flag_spawn",
        ObjectiveKind::FlagPlate => "flag_plate",
        ObjectiveKind::Zone => "zone",
        ObjectiveKind::BallStand => "ball_stand",
        ObjectiveKind::BombSpawn => "bomb_spawn",
        ObjectiveKind::BombSite => "bomb_site",
        _ => "?",
    }
}

#[test]
fn objectives_match_the_placements() {
    let Some(root) = root() else { return };
    let mut checked = 0;
    for d in downloads(&root) {
        let path = root.join("objectives").join(format!("{}.json", d.asset));
        if !path.exists() {
            assert!(d.map.objectives().is_empty(), "{}: objectives, and no placements", d.asset);
            continue;
        }
        let doc = json(&path);
        assert_eq!(doc["mapVersionId"], d.version.as_str(), "{}: placements of another version", d.asset);
        let want = doc["objects"].as_array().expect("objects");
        let got = d.map.objectives();
        assert_eq!(got.len(), want.len(), "{}: {got:?}", d.asset);
        for (o, w) in got.iter().zip(want) {
            // The files list a placement's modes by name.
            let mut modes: Vec<&str> = o.modes.iter().map(|&m| mode_name(m)).collect();
            modes.sort_unstable();
            let want_modes: Vec<&str> = w["modes"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
            let at = (o.position.x, o.position.y, o.position.z.unwrap());
            let mut ok = modes == want_modes
                && w["kind"] == kind_name(o.kind)
                && near(at.0, &w["x"], 0.0051)
                && near(at.1, &w["y"], 0.0051)
                && near(at.2, &w["z"], 0.0051)
                && w.get("team").and_then(Value::as_u64) == o.team.map(|t| u64::from(t.0))
                && w.get("order").and_then(Value::as_u64) == o.order.map(u64::from);
            match (o.shape, w.get("b")) {
                (Some(s), Some(b)) => {
                    let e = |i: usize, v: f64| near(v, &b[i], 0.051);
                    ok &= e(3, s.top) && e(4, s.bottom);
                    ok &= match s.area {
                        // A cylinder has no width.
                        ZoneArea::Cylinder { radius, .. } => {
                            b[0] == 2 && e(1, radius) && b[2] == 0.0 && w.get("yaw").is_none()
                        }
                        ZoneArea::Box { length, width, .. } => {
                            b[0] == 3 && e(1, length) && e(2, width) && near(s.angle, &w["yaw"], 0.00051)
                        }
                        _ => false,
                    };
                }
                (s, b) => ok &= s.is_none() && b.is_none(),
            }
            assert!(ok, "{}: {o:?}\n{w}", d.asset);
            checked += 1;
        }
    }
    assert!(checked > 0, "no map variant of {} has its placements", root.display());
}

#[cfg(feature = "serde")]
#[test]
fn map_variants_round_trip_through_json() {
    let Some(root) = root() else { return };
    for d in downloads(&root) {
        let text = serde_json::to_string(&d.map).unwrap();
        let back: MapVariant = serde_json::from_str(&text).unwrap();
        assert!(back == d.map, "{}", d.asset);
        let objectives = d.map.objectives();
        let back: Vec<filmshell::map_variant::Objective> =
            serde_json::from_str(&serde_json::to_string(&objectives).unwrap()).unwrap();
        assert_eq!(back, objectives, "{}", d.asset);
    }
}

#[test]
fn damaged_map_variants_never_panic() {
    let Some(root) = root() else { return };
    let Some(d) = downloads(&root).into_iter().min_by_key(|d| d.bytes.len()) else { return };
    let mut seed = 0x2545_f491_4f6c_dd1du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let n = d.bytes.len();
    for i in 0..300 {
        let mut b = d.bytes.clone();
        match i % 3 {
            0 => {
                for _ in 0..8 {
                    let at = next() as usize % n;
                    b[at] ^= 1 << (next() % 8);
                }
            }
            1 => b.truncate(next() as usize % n),
            _ => {
                let at = next() as usize % n;
                for x in b.iter_mut().skip(at).take(64) {
                    *x = next() as u8;
                }
            }
        }
        if let Ok(m) = MapVariant::from_bytes(&b) {
            let _ = m.objectives();
            let _ = m.spawn_pads().count();
        }
    }
}
