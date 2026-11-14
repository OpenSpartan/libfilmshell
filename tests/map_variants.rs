//! Map variants against the asset catalogue: the objects and objective
//! placements film-assets' Python tools read from the same files.
//!
//! film-assets is a private repository. `FILM_ASSETS_DIR` names a checkout;
//! the tests read its Forge downloads (`sources/forge/<asset>/map.mvar`
//! with `version.json`), its catalogue (`sources/catalogue.json`: every
//! gameplay object of each variant, by name) and its objective placements
//! (`objectives/<asset>.json`). Without the variable they check nothing.

use filmshell::map_variant::{MapObject, MapVariant, ObjectType, ObjectiveKind, ObjectiveMode};
use filmshell::{SpawnKind, StringId, ZoneArea, ZoneShape};
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

/// Every Forge download with its map variant, by asset id.
fn downloads(root: &Path) -> Vec<Download> {
    let forge = root.join("sources").join("forge");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&forge)
        .unwrap_or_else(|e| panic!("{}: {e}", forge.display()))
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.join("map.mvar").exists())
        .collect();
    dirs.sort();
    assert!(!dirs.is_empty(), "{VAR} is set, but {} holds no map variants", forge.display());
    dirs.iter()
        .map(|d| {
            let meta = ["version.json", "asset.json"].map(|n| d.join(n)).into_iter().find(|p| p.exists());
            let meta = json(&meta.unwrap_or_else(|| panic!("{}: no version.json", d.display())));
            let bytes = std::fs::read(d.join("map.mvar")).expect("a map variant");
            let map = MapVariant::from_bytes(&bytes).unwrap_or_else(|e| panic!("{}: {e}", d.display()));
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

/// The differences between an object and its catalogue row
/// (filmshell's `tools/hi/mvars.py`): positions rounded to hundredths, the
/// forward axis to thousandths and in engine axes, a volume's extents in
/// engine units.
fn differences(o: &MapObject, row: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut check = |ok: bool, what: &str| {
        if !ok {
            out.push(what.to_owned());
        }
    };
    let name = o.name().map_or_else(|| format!("?{}", o.tag), str::to_owned);
    check(row["name"] == name.as_str(), "name");
    check(row["type"] == o.object_type.raw(), "type");
    let z = o.position.z.unwrap();
    check(near(o.position.x, &row["x"], 0.0051) && near(o.position.y, &row["y"], 0.0051), "position");
    check(near(z, &row["z"], 0.0051), "height");
    // The catalogue's forward is the engine's x and y: world (y, -x).
    check(near(o.forward[1], &row["fwd"][0], 0.00051) && near(-o.forward[0], &row["fwd"][1], 0.00051), "forward");
    check(row.get("team").and_then(Value::as_u64) == o.team.map(|t| u64::from(t.0)), "team");
    check(row.get("spawnOrder").and_then(Value::as_u64) == o.spawn_order.map(u64::from), "spawn order");
    let labels: Vec<String> = o.labels.iter().map(StringId::to_string).collect();
    let row_labels: Vec<&str> = row
        .get("labels")
        .and_then(Value::as_array)
        .map_or(Vec::new(), |l| l.iter().filter_map(Value::as_str).collect());
    check(labels == row_labels, "labels");
    check(row.get("respawnS").and_then(Value::as_u64) == o.respawn_time.map(u64::from), "respawn time");
    // The catalogue names a variant by the game's string table, else by its
    // id; 00000000 is none.
    let variant = row.get("variant").and_then(Value::as_str).filter(|v| *v != "00000000").map(|v| {
        if v.len() == 8 && v.bytes().all(|b| b.is_ascii_hexdigit()) { v.parse().unwrap() } else { StringId::of(v) }
    });
    check(variant == o.variant, "variant");
    match (&row.get("bound"), o.boundary) {
        (Some(b), Some(ZoneShape { area, top, bottom, .. })) => {
            let e = |k: &str, v: f64| near(v / 10.0, &b[k], 1e-9);
            let ok = match area {
                // A cylinder's second extent is the box's width it was
                // made from, which nothing uses.
                ZoneArea::Cylinder { radius, .. } => b["kind"] == 2 && e("a", radius),
                ZoneArea::Box { length, width, .. } => b["kind"] == 3 && e("a", length) && e("b", width),
                _ => false,
            };
            check(ok && e("up", top) && e("down", bottom), "boundary");
        }
        (Some(b), None) => check(b["kind"] != 2 && b["kind"] != 3, "boundary"),
        (None, b) => check(b.is_none(), "boundary"),
    }
    if o.object_type == ObjectType::SpawnPoint {
        let respawn = row["sp"]["0"] == false;
        check(o.spawn == Some(if respawn { SpawnKind::Respawn } else { SpawnKind::Initial }), "spawn kind");
    } else {
        check(o.spawn.is_none(), "spawn kind");
    }
    out
}

#[test]
fn objects_match_the_catalogue() {
    let Some(root) = root() else { return };
    let catalogue = json(&root.join("sources").join("catalogue.json"));
    let rows = catalogue.as_array().expect("a list of map variants");
    let mut checked = 0;
    for d in downloads(&root) {
        let Some(row) =
            rows.iter().find(|r| r["mapAssetId"] == d.asset.as_str() && r["mapVersionId"] == d.version.as_str())
        else {
            continue;
        };
        assert_eq!(row["levelId"], d.map.level.to_string().as_str(), "{}", d.asset);
        assert_eq!(row["n"].as_u64(), Some(d.map.objects.len() as u64), "{}", d.asset);
        // The catalogue keeps the gameplay objects, in the file's order.
        let listed = row["objects"].as_array().expect("objects");
        let mut next = listed.iter().peekable();
        for o in &d.map.objects {
            let Some(r) = next.peek() else { break };
            if r["tag"] != o.tag.to_string().as_str() || !near(o.position.x, &r["x"], 0.0051) {
                continue;
            }
            let diff = differences(o, r);
            assert!(diff.is_empty(), "{} {}: {diff:?}\n{o:?}\n{r}", d.asset, o.tag);
            next.next();
            checked += 1;
        }
        assert!(next.peek().is_none(), "{}: catalogue objects not in the map variant, from {:?}", d.asset, next.peek());
    }
    assert!(checked > 0, "no map variant of {} is in its catalogue", root.display());
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
        if doc["mapVersionId"] != d.version.as_str() {
            continue;
        }
        let want = doc["objects"].as_array().expect("objects");
        let got = d.map.objectives();
        assert_eq!(got.len(), want.len(), "{}: {got:?}", d.asset);
        for (o, w) in got.iter().zip(want) {
            let modes: Vec<&str> = o
                .modes
                .iter()
                .map(|m| match m {
                    ObjectiveMode::CaptureTheFlag => "ctf",
                    ObjectiveMode::KingOfTheHill => "koth",
                    ObjectiveMode::Strongholds => "strongholds",
                    ObjectiveMode::LandGrab => "landgrab",
                    ObjectiveMode::TotalControl => "totalcontrol",
                    ObjectiveMode::Extraction => "extraction",
                    ObjectiveMode::Oddball => "oddball",
                    ObjectiveMode::Assault => "assault",
                    _ => "?",
                })
                .collect();
            let mut want_modes: Vec<&str> = w["modes"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
            let mut sorted = modes.clone();
            sorted.sort();
            want_modes.sort();
            let kind = match o.kind {
                ObjectiveKind::FlagSpawn => "flag_spawn",
                ObjectiveKind::FlagPlate => "flag_plate",
                ObjectiveKind::Zone => "zone",
                ObjectiveKind::BallStand => "ball_stand",
                ObjectiveKind::BombSpawn => "bomb_spawn",
                ObjectiveKind::BombSite => "bomb_site",
                _ => "?",
            };
            let at = (o.position.x, o.position.y, o.position.z.unwrap());
            let mut ok = sorted == want_modes
                && w["kind"] == kind
                && near(at.0, &w["x"], 0.0051)
                && near(at.1, &w["y"], 0.0051)
                && near(at.2, &w["z"], 0.0051)
                && w.get("team").and_then(Value::as_u64) == o.team.map(|t| u64::from(t.0))
                && w.get("order").and_then(Value::as_u64) == o.order.map(u64::from);
            // The volume to tenths, a box's heading to hundredths or thousandths
            // (the files differ).
            match (o.shape, w.get("b")) {
                (Some(s), Some(b)) => {
                    let e = |i: usize, v: f64| near(v, &b[i], 0.051);
                    ok &= e(3, s.top) && e(4, s.bottom);
                    ok &= match s.area {
                        ZoneArea::Cylinder { radius, .. } => b[0] == 2 && e(1, radius) && w.get("yaw").is_none(),
                        ZoneArea::Box { length, width, .. } => {
                            b[0] == 3 && e(1, length) && e(2, width) && near(s.angle, &w["yaw"], 0.0051)
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
