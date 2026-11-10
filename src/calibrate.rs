//! Raw positions to world units.
//!
//! Each level quantizes each horizontal axis as an N-bit field over its own
//! box. Read as 15-bit windows anchored at the field MSB, the bit count
//! divides out:
//!
//! ```text
//! world_y =  sy * (c1 - 16384) + dy        sy = R_y / 32768
//! world_x = -sx * (c2 - 16384) + dx        sx = R_x / 32768
//! ```
//!
//! Known boxes live in `FORGE_CANVAS` and `KNOWN_LAYOUTS`; unknown levels are
//! searched from life starts against spawn pads.

use crate::levels::LevelBox;
use crate::types::*;

const DEFAULT_SCALE: f64 = 0.003;
const COMPOSITE_RAW_CENTER: f64 = 16384.0;

#[derive(Clone, Copy, Debug)]
pub struct WindowCalibration {
    pub sy: f64,
    pub sx: f64,
    pub dx: f64,
    pub dy: f64,
}

/// The box every Forge canvas shares (c2 starts at bit 22).
pub const FORGE_CANVAS: WindowCalibration = WindowCalibration { sy: 0.14122, sx: 0.13844, dx: 3.6, dy: 3.25 };
const FORGE_C2_START: i64 = 22;

/// Studio levels' boxes, by map variant asset id, with the c2 start bit they
/// apply at.
fn known_layout(asset_id: &str) -> Option<(i64, WindowCalibration)> {
    match asset_id {
        // Aquarius.
        "c395f3ac-4614-45f9-a83a-56f69e8ae962" => {
            Some((20, WindowCalibration { sy: 0.02375, sx: 0.01411, dx: 47.5, dy: -1.1 }))
        }
        // Illusion.
        "9e821f5e-042f-407c-97f3-de165b1cdb26" => {
            Some((25, WindowCalibration { sy: 0.7534, sx: 0.6900, dx: -0.4, dy: 0.3 }))
        }
        // Fragmentation Heavies.
        "0d849a52-fedb-4aea-b5a3-caee268f1f49" => {
            Some((24, WindowCalibration { sy: 0.5130, sx: 0.4537, dx: -409.3, dy: -2966.4 }))
        }
        _ => None,
    }
}

/// Frames between two on-foot samples taken as a respawn when lives aren't
/// marked.
const RESPAWN_GAP: i64 = 120;
/// Inlier radius, world units, for a spawn sample to count as on a pad.
const PAD_INLIER: f64 = 5.0;

#[derive(Clone, Copy)]
struct Sample {
    r1: f64,
    r2: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct Pad {
    pub x: f64,
    pub y: f64,
}

/// Spawn evidence: each path's first sample plus the first sample of every
/// later life, centred on the window midpoint.
fn collect_spawn_samples(paths: &[&PlayerPath]) -> Vec<Sample> {
    let mut out = Vec::new();
    let push = |out: &mut Vec<Sample>, m: &MotionPoint| {
        out.push(Sample { r1: m.raw1 - COMPOSITE_RAW_CENTER, r2: m.raw2 - COMPOSITE_RAW_CENTER })
    };
    for p in paths {
        let pos = &p.positions;
        if pos.is_empty() {
            continue;
        }
        push(&mut out, &pos[0]);
        if let Some(starts) = &p.life_starts {
            for &i in starts {
                if i > 0 && i < pos.len() && !pos[i].in_vehicle {
                    push(&mut out, &pos[i]);
                }
            }
            continue;
        }
        if let Some(ls) = &p.life_slots {
            let k = ls.len();
            if k < 2 {
                continue;
            }
            let mut gaps: Vec<(usize, i64)> = (1..pos.len()).map(|i| (i, pos[i].frame - pos[i - 1].frame)).collect();
            gaps.sort_by_key(|g| std::cmp::Reverse(g.1));
            for &(i, _) in gaps.iter().take(k - 1) {
                if !pos[i].in_vehicle {
                    push(&mut out, &pos[i]);
                }
            }
            continue;
        }
        for i in 1..pos.len() {
            let (a, b) = (&pos[i - 1], &pos[i]);
            if a.in_vehicle || b.in_vehicle {
                continue;
            }
            if b.frame - a.frame >= RESPAWN_GAP {
                push(&mut out, b);
            }
        }
    }
    out
}

fn nearest_pad_dist(x: f64, y: f64, pads: &[Pad]) -> (f64, Pad) {
    let mut bd = f64::INFINITY;
    let mut bp = pads.first().copied().unwrap_or(Pad { x: f64::NAN, y: f64::NAN });
    for p in pads {
        let (dx, dy) = (p.x - x, p.y - y);
        let d = dx * dx + dy * dy;
        if d < bd {
            bd = d;
            bp = *p;
        }
    }
    (bd.sqrt(), bp)
}

#[derive(Clone, Copy)]
struct FitStats {
    inliers: usize,
    n: usize,
    cost: f64,
}

fn fit_stats(s: &[Sample], pads: &[Pad], f: &WindowCalibration) -> FitStats {
    let (mut inliers, mut cost) = (0usize, 0.0f64);
    for x in s {
        let (d, _) = nearest_pad_dist(-f.sx * x.r2 + f.dx, f.sy * x.r1 + f.dy, pads);
        if d < PAD_INLIER {
            inliers += 1;
        }
        cost += f64::min(d, 15.0);
    }
    FitStats { inliers, n: s.len(), cost: if s.is_empty() { f64::INFINITY } else { cost / s.len() as f64 } }
}

/// Translation for a fixed scale: Hough vote over sample-to-pad offsets
/// within `max_shift`, then a shrinking-radius mean-residual refine.
fn refine_translation(s: &[Sample], pads: &[Pad], base: &WindowCalibration, max_shift: f64) -> Option<(f64, f64)> {
    if s.len() < 6 || pads.is_empty() {
        return None;
    }
    let cell = 3.0;
    let nb = ((2.0 * max_shift) / cell).ceil() as usize + 2;
    let mut votes = vec![0i32; nb * nb];
    for x in s {
        let (wx, wy) = (-base.sx * x.r2 + base.dx, base.sy * x.r1 + base.dy);
        for p in pads {
            let (tx, ty) = (p.x - wx, p.y - wy);
            if tx < -max_shift || tx >= max_shift || ty < -max_shift || ty >= max_shift {
                continue;
            }
            let i = ((tx + max_shift) / cell).floor() as usize;
            let j = ((ty + max_shift) / cell).floor() as usize;
            votes[i * nb + j] += 1;
        }
    }
    let (mut best, mut bi, mut bj) = (-1i32, 0usize, 0usize);
    for i in 0..nb - 1 {
        for j in 0..nb - 1 {
            let v = votes[i * nb + j] + votes[(i + 1) * nb + j] + votes[i * nb + j + 1] + votes[(i + 1) * nb + j + 1];
            if v > best {
                best = v;
                bi = i;
                bj = j;
            }
        }
    }
    let mut dx = base.dx + (bi as f64 + 1.0) * cell - max_shift;
    let mut dy = base.dy + (bj as f64 + 1.0) * cell - max_shift;
    for thr in [8.0, 6.0, PAD_INLIER, PAD_INLIER, PAD_INLIER] {
        let (mut ex, mut ey, mut n) = (0.0f64, 0.0f64, 0usize);
        for x in s {
            let (wx, wy) = (-base.sx * x.r2 + dx, base.sy * x.r1 + dy);
            let (d, p) = nearest_pad_dist(wx, wy, pads);
            if d < thr {
                ex += p.x - wx;
                ey += p.y - wy;
                n += 1;
            }
        }
        if n < 6 {
            return None;
        }
        dx += ex / n as f64;
        dy += ey / n as f64;
    }
    if (dx - base.dx).abs() > max_shift || (dy - base.dy).abs() > max_shift {
        return None;
    }
    Some((dx, dy))
}

/// Shrinking-radius robust least squares for all four window parameters.
fn refine_affine(s: &[Sample], pads: &[Pad], init: &WindowCalibration) -> Option<WindowCalibration> {
    let WindowCalibration { mut sy, mut sx, mut dx, mut dy } = *init;
    for thr in [24.0, 16.0, 10.0, 7.0, PAD_INLIER, PAD_INLIER, PAD_INLIER] {
        let (mut r1, mut r2, mut px, mut py) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
        for x in s {
            let (d, p) = nearest_pad_dist(-sx * x.r2 + dx, sy * x.r1 + dy, pads);
            if d < thr {
                r1.push(x.r1);
                r2.push(x.r2);
                px.push(p.x);
                py.push(p.y);
            }
        }
        if r1.len() < 6 {
            return None;
        }
        let lin = |u: &[f64], v: &[f64], slope: f64| -> (f64, f64) {
            let n = u.len() as f64;
            let mu = u.iter().fold(0.0, |a, b| a + b) / n;
            let mv = v.iter().fold(0.0, |a, b| a + b) / n;
            let (mut suu, mut suv, mut vmin, mut vmax) = (0.0f64, 0.0f64, f64::INFINITY, f64::NEG_INFINITY);
            for i in 0..u.len() {
                suu += f64::powf(u[i] - mu, 2.0);
                suv += (u[i] - mu) * (v[i] - mv);
                vmin = f64::min(vmin, v[i]);
                vmax = f64::max(vmax, v[i]);
            }
            let a = if vmax - vmin >= 20.0 && suu > 0.0 { suv / suu } else { slope };
            (a, mv - a * mu)
        };
        let (a, b) = lin(&r1, &py, sy);
        sy = a;
        dy = b;
        let (neg_sx, ox) = lin(&r2, &px, -sx);
        sx = -neg_sx;
        dx = ox;
        // NaN fails too.
        if sy.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
            || sx.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater)
        {
            return None;
        }
    }
    Some(WindowCalibration { sy, sx, dx, dy })
}

/// Median distance, world units, from each pad to the nearest path sample, capped at 60.
fn pad_coverage(pts: &[Sample], pads: &[Pad], f: &WindowCalibration) -> f64 {
    let cell = 6.0;
    let mut grid: std::collections::HashMap<(i64, i64), Vec<f64>> = std::collections::HashMap::new();
    for s in pts {
        let (x, y) = (-f.sx * s.r2 + f.dx, f.sy * s.r1 + f.dy);
        let key = ((x / cell).floor() as i64, (y / cell).floor() as i64);
        let a = grid.entry(key).or_default();
        a.push(x);
        a.push(y);
    }
    let mut ds: Vec<f64> = Vec::new();
    for p in pads {
        // Coordinates past any map's would overflow the cell arithmetic.
        if !(p.x.abs() < 1e9 && p.y.abs() < 1e9) {
            continue;
        }
        let (ci, cj) = ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
        let mut best = 60.0f64;
        let mut r = 0i64;
        while (r as f64) * cell <= best + cell && r <= 10 {
            for i in ci - r..=ci + r {
                for j in cj - r..=cj + r {
                    if (i - ci).abs().max((j - cj).abs()) != r {
                        continue;
                    }
                    let Some(a) = grid.get(&(i, j)) else { continue };
                    let mut q = 0;
                    while q < a.len() {
                        let d = f64::hypot(a[q] - p.x, a[q + 1] - p.y);
                        if d < best {
                            best = d;
                        }
                        q += 2;
                    }
                }
            }
            r += 1;
        }
        ds.push(best);
    }
    ds.sort_by(f64::total_cmp);
    if ds.is_empty() {
        60.0
    } else {
        ds[ds.len() / 2]
    }
}

/// Search an unknown level's box over a log grid of (sy, sy/sx), Hough
/// translation per candidate, the best 12 refined in all four parameters.
fn search_layout(s: &[Sample], pads: &[Pad], pts: &[Sample], c2_start: i64) -> Option<WindowCalibration> {
    if s.len() < 6 || pads.len() < 2 {
        return None;
    }
    let sub: Vec<Sample> = if s.len() > 120 {
        let k = (s.len() as f64 / 120.0).ceil() as usize;
        s.iter().enumerate().filter(|(i, _)| i % k == 0).map(|(_, x)| *x).collect()
    } else {
        s.to_vec()
    };
    let (mut pmin_x, mut pmax_x, mut pmin_y, mut pmax_y, mut pcx, mut pcy) =
        (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY, 0.0f64, 0.0f64);
    for p in pads {
        pmin_x = f64::min(pmin_x, p.x);
        pmax_x = f64::max(pmax_x, p.x);
        pmin_y = f64::min(pmin_y, p.y);
        pmax_y = f64::max(pmax_y, p.y);
        pcx += p.x / pads.len() as f64;
        pcy += p.y / pads.len() as f64;
    }
    let sc1 = sub.iter().fold(0.0, |a, x| a + x.r1) / sub.len() as f64;
    let sc2 = sub.iter().fold(0.0, |a, x| a + x.r2) / sub.len() as f64;
    let half_w = (pmax_x - pmin_x) / 2.0 + 20.0;
    let half_h = (pmax_y - pmin_y) / 2.0 + 20.0;
    let cell = f64::max(4.0, f64::max(half_w, half_h) / 100.0);
    let nx = ((2.0 * half_w) / cell).ceil() as usize + 1;
    let ny = ((2.0 * half_h) / cell).ceil() as usize + 1;
    let mut votes = vec![0i32; nx * ny];

    let base = f64::powf(2.0, (c2_start - 7 - 15) as f64);
    let (lo, hi) = (0.05 * base, 0.35 * base);
    let steps = (f64::ln(hi / lo) / f64::ln(1.03)).ceil() as i64;
    let mut cands: Vec<(WindowCalibration, f64)> = Vec::new();
    for k in 0..=steps {
        let sy = lo * f64::powf(hi / lo, k as f64 / steps as f64);
        for q in -8i32..=8 {
            let sx = sy / f64::powf(2.0, q as f64 / 8.0);
            let (cx, cy) = (pcx + sx * sc2, pcy - sy * sc1);
            votes.iter_mut().for_each(|v| *v = 0);
            for x in &sub {
                let (wx, wy) = (-sx * x.r2, sy * x.r1);
                for p in pads {
                    let i = ((p.x - wx - cx + half_w) / cell).floor();
                    let j = ((p.y - wy - cy + half_h) / cell).floor();
                    if i >= 0.0 && i < nx as f64 && j >= 0.0 && j < ny as f64 {
                        votes[i as usize * ny + j as usize] += 1;
                    }
                }
            }
            let (mut best_v, mut bi, mut bj) = (-1i32, 0usize, 0usize);
            for i in 0..nx - 1 {
                for j in 0..ny - 1 {
                    let v = votes[i * ny + j]
                        + votes[(i + 1) * ny + j]
                        + votes[i * ny + j + 1]
                        + votes[(i + 1) * ny + j + 1];
                    if v > best_v {
                        best_v = v;
                        bi = i;
                        bj = j;
                    }
                }
            }
            let f = WindowCalibration {
                sy,
                sx,
                dx: cx - half_w + (bi as f64 + 1.0) * cell,
                dy: cy - half_h + (bj as f64 + 1.0) * cell,
            };
            let cost = fit_stats(&sub, pads, &f).cost;
            cands.push((f, cost));
        }
    }
    cands.sort_by(|a, b| f64::total_cmp(&a.1, &b.1));
    let mut best: Option<(WindowCalibration, f64)> = None;
    for (c, _) in cands.iter().take(12) {
        let Some(f) = refine_affine(s, pads, c) else { continue };
        if pad_coverage(pts, pads, &f) > 25.0 {
            continue;
        }
        let cost = fit_stats(s, pads, &f).cost;
        if best.as_ref().is_none_or(|b| cost < b.1) {
            best = Some((f, cost));
        }
    }
    best.map(|b| b.0)
}

/// Last resort: map the path network's 2nd to 98th percentile extent onto the pad box.
fn extent_fit(pts: &[Sample], pads: &[Pad]) -> Option<WindowCalibration> {
    if pts.len() < 50 || pads.len() < 2 {
        return None;
    }
    let q = |a: &[f64], f: f64| a[(a.len() - 1).min((f * a.len() as f64).floor() as usize)];
    let mut r1: Vec<f64> = pts.iter().map(|s| s.r1).collect();
    r1.sort_by(f64::total_cmp);
    let mut r2: Vec<f64> = pts.iter().map(|s| s.r2).collect();
    r2.sort_by(f64::total_cmp);
    let maxv = |v: &mut dyn Iterator<Item = f64>| v.fold(f64::NEG_INFINITY, f64::max);
    let minv = |v: &mut dyn Iterator<Item = f64>| v.fold(f64::INFINITY, f64::min);
    let (xmax, xmin) = (maxv(&mut pads.iter().map(|p| p.x)), minv(&mut pads.iter().map(|p| p.x)));
    let (ymax, ymin) = (maxv(&mut pads.iter().map(|p| p.y)), minv(&mut pads.iter().map(|p| p.y)));
    let (w, h) = (xmax - xmin, ymax - ymin);
    let e1 = q(&r1, 0.98) - q(&r1, 0.02);
    let e2 = q(&r2, 0.98) - q(&r2, 0.02);
    if e1 <= 0.0 || e2 <= 0.0 || w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (sy, sx) = (h / e1, w / e2);
    Some(WindowCalibration {
        sy,
        sx,
        dy: (ymax + ymin) / 2.0 - sy * (q(&r1, 0.98) + q(&r1, 0.02)) / 2.0,
        dx: (xmax + xmin) / 2.0 + sx * (q(&r2, 0.98) + q(&r2, 0.02)) / 2.0,
    })
}

/// Evenly strided subsample of every position for the coverage term.
fn sample_path_points(paths: &[&PlayerPath]) -> Vec<Sample> {
    let max = 4000usize;
    let total: usize = paths.iter().map(|p| p.positions.len()).sum();
    let stride = 1.max(total / max);
    let mut out = Vec::new();
    let mut i = 0usize;
    for p in paths {
        for pt in &p.positions {
            if i % stride == 0 {
                out.push(Sample { r1: pt.raw1 - COMPOSITE_RAW_CENTER, r2: pt.raw2 - COMPOSITE_RAW_CENTER });
            }
            i += 1;
        }
    }
    out
}

fn window_to_calibration(f: &WindowCalibration, source: CalibSource, st: Option<FitStats>) -> Calibration {
    Calibration {
        scale_y: f.sy,
        scale_x: f.sx,
        offset_y: f.dy - f.sy * COMPOSITE_RAW_CENTER,
        offset_x: f.dx + f.sx * COMPOSITE_RAW_CENTER,
        source,
        spawn_inliers: st.map(|s| s.inliers),
        spawn_samples: st.map(|s| s.n),
    }
}

/// World units per engine unit.
pub(crate) const WORLD_PER_ENGINE: f64 = 10.0;
/// Whether a quantized value converts to the middle of its step rather than
/// its start.
const MID_STEP: bool = true;

/// A catalogued level's box as a window calibration. Window w (field value
/// times 2^(15 - N)) on an axis of extent E is engine min + E * w / 32768; c1
/// is engine X (world Y), c2 engine Y (world X = -10 * engine Y).
fn level_window(b: &LevelBox, mid_step: bool) -> WindowCalibration {
    let sy = (b.max[0] - b.min[0]) * WORLD_PER_ENGINE / 32768.0;
    let sx = (b.max[1] - b.min[1]) * WORLD_PER_ENGINE / 32768.0;
    let half = |w: u32| if mid_step { 0.5 * 2.0f64.powi(15 - w as i32) } else { 0.0 };
    WindowCalibration {
        sy,
        sx,
        dy: b.min[0] * WORLD_PER_ENGINE + sy * (COMPOSITE_RAW_CENTER + half(b.widths[0])),
        dx: -b.min[1] * WORLD_PER_ENGINE - sx * (COMPOSITE_RAW_CENTER + half(b.widths[1])),
    }
}

/// The catalogued box applies only when the film's fields have the widths
/// the box implies: a different build or a wrong level id falls through to
/// the fitted paths.
pub(crate) fn level_matches(b: &LevelBox, c2_start: i64, c2_width: Option<u32>) -> bool {
    b.widths[0] as i64 == c2_start - b.c1_at() as i64 && c2_width.is_none_or(|w| w == b.widths[1])
}

/// Multiplayer-composite calibration (raw = 15-bit windows).
fn calibrate_composite(
    human_paths: &[&PlayerPath],
    pads: &[Pad],
    asset_id: Option<&str>,
    coord_layout: Option<i64>,
    level: Option<&LevelBox>,
    c2_width: Option<u32>,
) -> Option<Calibration> {
    let c2_start = coord_layout.unwrap_or(FORGE_C2_START);
    let s = collect_spawn_samples(human_paths);
    if let Some(b) = level.filter(|b| level_matches(b, c2_start, c2_width)) {
        let f = level_window(b, MID_STEP);
        let st = (!pads.is_empty()).then(|| fit_stats(&s, pads, &f));
        return Some(window_to_calibration(&f, CalibSource::LevelTable, st));
    }
    let known = asset_id.and_then(known_layout);
    let (base, base_source, is_forge) = match known {
        Some((seam, w)) if seam == c2_start => (Some(w), CalibSource::AssetTable, false),
        _ if c2_start == FORGE_C2_START => (Some(FORGE_CANVAS), CalibSource::ForgeCanvas, true),
        _ => (None, CalibSource::ForgeCanvas, false),
    };

    if let Some(base) = base {
        let f = match refine_translation(&s, pads, &base, 40.0) {
            Some((dx, dy)) => WindowCalibration { dx, dy, ..base },
            None => base,
        };
        let st = if pads.is_empty() { None } else { Some(fit_stats(&s, pads, &f)) };
        let weak = st.is_some_and(|st| st.n >= 20 && (st.inliers as f64) < 0.12 * st.n as f64);
        if !weak || !is_forge {
            return Some(window_to_calibration(&f, base_source, st));
        }
        let st = st.unwrap();
        if let Some(g) = search_layout(&s, pads, &sample_path_points(human_paths), c2_start) {
            let gs = fit_stats(&s, pads, &g);
            if gs.inliers >= 10 && gs.inliers >= 2 * st.inliers {
                return Some(window_to_calibration(&g, CalibSource::SpawnFit, Some(gs)));
            }
        }
        return Some(window_to_calibration(&f, base_source, Some(st)));
    }

    let pts = sample_path_points(human_paths);
    if let Some(g) = search_layout(&s, pads, &pts, c2_start) {
        let gs = fit_stats(&s, pads, &g);
        if gs.inliers as f64 >= f64::max(8.0, 0.15 * gs.n as f64) {
            return Some(window_to_calibration(&g, CalibSource::SpawnFit, Some(gs)));
        }
    }
    extent_fit(&pts, pads).map(|e| window_to_calibration(&e, CalibSource::ExtentFit, Some(fit_stats(&s, pads, &e))))
}

/// A known window box in small-lobby units (raw1 = 0x4000 + window/64 with
/// the low bits below c1 zeroed, raw2 = window >> 3).
fn per_entity_from_window(f: &WindowCalibration, seam: i64, source: CalibSource) -> Calibration {
    let y_mid = (f64::powf(2.0, 0.max(FORGE_C2_START - seam) as f64) - 1.0) / 2.0;
    Calibration {
        scale_y: 64.0 * f.sy,
        offset_y: f.dy + f.sy * (y_mid - 64.0 * 16384.0 - COMPOSITE_RAW_CENTER),
        scale_x: 8.0 * f.sx,
        offset_x: f.dx + f.sx * (COMPOSITE_RAW_CENTER - 3.5),
        source,
        spawn_inliers: None,
        spawn_samples: None,
    }
}

/// 8 times Aquarius' tall-to-wide box ratio: scale_y / scale_x in small-lobby units
/// on a level with no known box.
const PER_ENTITY_RATIO: f64 = 8.0 * (0.02375 / 0.01411);
const DEFAULT_SCALE_Y: f64 = 1.48;

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

fn calibrate_per_entity(with_data: &[&PlayerPath], initial_spawns: &[Pad], b: &MapBounds) -> Option<Calibration> {
    let raw_starts: Vec<(f64, f64)> = with_data.iter().map(|p| (p.positions[0].raw1, p.positions[0].raw2)).collect();
    let mk = |scale_y: f64, scale_x: f64, offset_y: f64, offset_x: f64, source: CalibSource| Calibration {
        scale_y,
        scale_x,
        offset_y,
        offset_x,
        source,
        spawn_inliers: None,
        spawn_samples: None,
    };

    if with_data.len() >= 2 && initial_spawns.len() >= 2 {
        let (mut ai, mut bi, mut best_d1, mut best_d2) = (0usize, 1usize, -1.0f64, -1.0f64);
        for i in 0..raw_starts.len() {
            for j in i + 1..raw_starts.len() {
                let d1 = (raw_starts[j].0 - raw_starts[i].0).abs();
                let d2 = (raw_starts[j].1 - raw_starts[i].1).abs();
                if f64::max(d1, d2) > f64::max(best_d1, best_d2) {
                    best_d1 = d1;
                    best_d2 = d2;
                    ai = i;
                    bi = j;
                }
            }
        }
        let (ra, rb) = (raw_starts[ai], raw_starts[bi]);
        let d_raw1 = rb.0 - ra.0;
        let d_raw2 = rb.1 - ra.1;
        let y_primary = d_raw1.abs() >= d_raw2.abs();
        let d_raw_p = if y_primary { d_raw1 } else { d_raw2 };
        if d_raw_p.abs() > 10.0 {
            let (mut min_r1, mut max_r1, mut min_r2, mut max_r2) =
                (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
            for p in with_data {
                for pt in &p.positions {
                    min_r1 = f64::min(min_r1, pt.raw1);
                    max_r1 = f64::max(max_r1, pt.raw1);
                    min_r2 = f64::min(min_r2, pt.raw2);
                    max_r2 = f64::max(max_r2, pt.raw2);
                }
            }
            let mut best: Option<Calibration> = None;
            let mut best_score = f64::INFINITY;
            for (ia, sa) in initial_spawns.iter().enumerate() {
                for (ib, sb) in initial_spawns.iter().enumerate() {
                    if ia == ib {
                        continue;
                    }
                    let (dy, dx) = (sb.y - sa.y, sb.x - sa.x);
                    let (scale_y, scale_x);
                    if y_primary {
                        if sign(dy) != sign(d_raw1) {
                            continue;
                        }
                        scale_y = dy / d_raw1;
                        if scale_y <= 0.0 {
                            continue;
                        }
                        scale_x = if d_raw2.abs() > 10.0 && dx.abs() > 1.0 {
                            (dx / d_raw2).abs()
                        } else {
                            scale_y / PER_ENTITY_RATIO
                        };
                    } else {
                        if sign(dx) != -sign(d_raw2) {
                            continue;
                        }
                        scale_x = (dx / d_raw2).abs();
                        if scale_x <= 0.0 {
                            continue;
                        }
                        scale_y = if d_raw1.abs() > 10.0 && dy.abs() > 1.0 {
                            (dy / d_raw1).abs()
                        } else {
                            scale_x * PER_ENTITY_RATIO
                        };
                    }
                    let offset_y = sa.y - scale_y * ra.0;
                    let offset_x = sa.x + scale_x * ra.1;
                    let pred_by = scale_y * rb.0 + offset_y;
                    let pred_bx = -scale_x * rb.1 + offset_x;
                    let err_b = if y_primary { (pred_bx - sb.x).abs() } else { (pred_by - sb.y).abs() };
                    let wy_min = scale_y * min_r1 + offset_y;
                    let wy_max = scale_y * max_r1 + offset_y;
                    let wx_min = -scale_x * max_r2 + offset_x;
                    let wx_max = -scale_x * min_r2 + offset_x;
                    let overflow = f64::max(0.0, b.min_y - wy_min)
                        + f64::max(0.0, wy_max - b.max_y)
                        + f64::max(0.0, b.min_x - wx_min)
                        + f64::max(0.0, wx_max - b.max_x);
                    let sep = if y_primary { dy.abs() } else { dx.abs() };
                    let score = err_b + overflow * 10.0 - sep;
                    if score < best_score {
                        best_score = score;
                        best = Some(mk(scale_y, scale_x, offset_y, offset_x, CalibSource::TwoSpawn));
                    }
                }
            }
            if best.is_some() {
                return best;
            }
        }
    }

    if initial_spawns.is_empty() {
        return None;
    }
    let p0 = with_data[0];
    let ra = raw_starts[0];
    let (mut min_r1, mut max_r1, mut min_r2, mut max_r2) =
        (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for pt in &p0.positions {
        min_r1 = f64::min(min_r1, pt.raw1);
        max_r1 = f64::max(max_r1, pt.raw1);
        min_r2 = f64::min(min_r2, pt.raw2);
        max_r2 = f64::max(max_r2, pt.raw2);
    }
    let (rel_max_r1, rel_min_r1) = (max_r1 - ra.0, min_r1 - ra.0);
    let (rel_max_r2, rel_min_r2) = (max_r2 - ra.1, min_r2 - ra.1);
    let mut best_spawn = initial_spawns[0];
    let mut best_scale_y = 0.0f64;
    for s in initial_spawns {
        let mut max_sy = f64::INFINITY;
        if rel_max_r1 > 0.0 {
            max_sy = f64::min(max_sy, (b.max_y - s.y) / rel_max_r1);
        }
        if rel_min_r1 < 0.0 {
            max_sy = f64::min(max_sy, (b.min_y - s.y) / rel_min_r1);
        }
        if rel_max_r2 > 0.0 {
            max_sy = f64::min(max_sy, (s.x - b.min_x) * PER_ENTITY_RATIO / rel_max_r2);
        }
        if rel_min_r2 < 0.0 {
            max_sy = f64::min(max_sy, (s.x - b.max_x) * PER_ENTITY_RATIO / rel_min_r2);
        }
        if !max_sy.is_finite() || max_sy <= 0.0 {
            continue;
        }
        let sy = f64::min(DEFAULT_SCALE_Y, max_sy);
        if sy > best_scale_y {
            best_scale_y = sy;
            best_spawn = *s;
        }
    }
    if best_scale_y <= 0.0 {
        return None;
    }
    let scale_x = best_scale_y / PER_ENTITY_RATIO;
    Some(mk(
        best_scale_y,
        scale_x,
        best_spawn.y - best_scale_y * ra.0,
        best_spawn.x + scale_x * ra.1,
        CalibSource::SingleSpawn,
    ))
}

/// Derive the raw-to-world transform from spawn evidence. `composite` selects
/// window units (multiplayer-composite) vs small-lobby units.
pub fn calibrate_from_spawns(
    human_paths: &[&PlayerPath],
    initial_spawns: &[Pad],
    map_bounds: Option<&MapBounds>,
    composite: bool,
    asset_id: Option<&str>,
    coord_layout: Option<i64>,
    level: Option<&LevelBox>,
    c2_width: Option<u32>,
) -> Option<Calibration> {
    let with_data: Vec<&PlayerPath> = human_paths.iter().copied().filter(|p| !p.positions.is_empty()).collect();
    if with_data.is_empty() {
        return None;
    }
    if composite {
        return calibrate_composite(&with_data, initial_spawns, asset_id, coord_layout, level, c2_width);
    }
    let known = asset_id.and_then(known_layout);
    let seam = coord_layout.or(known.map(|k| k.0));
    // Small-lobby units carry their own step offsets (per_entity_from_window).
    if let (Some(b), Some(seam)) = (level, seam) {
        if level_matches(b, seam, None) {
            return Some(per_entity_from_window(&level_window(b, false), seam, CalibSource::LevelTable));
        }
    }
    if let Some((ks, w)) = known {
        if Some(ks) == seam {
            return Some(per_entity_from_window(&w, ks, CalibSource::AssetTable));
        }
    }
    if seam == Some(FORGE_C2_START) {
        return Some(per_entity_from_window(&FORGE_CANVAS, FORGE_C2_START, CalibSource::ForgeCanvas));
    }
    let b = map_bounds?;
    if b.width <= 0.0 {
        return None;
    }
    calibrate_per_entity(&with_data, initial_spawns, b)
}

pub fn apply_calibration(positions: &[MotionPoint], c: &Calibration) -> Vec<WorldPosition> {
    positions
        .iter()
        .map(|p| WorldPosition {
            x: -c.scale_x * p.raw2 + c.offset_x,
            y: c.scale_y * p.raw1 + c.offset_y,
            frame: p.frame,
            in_vehicle: p.in_vehicle,
            vehicle: p.vehicle,
        })
        .collect()
}

/// No spawn or map context: centre the raw extent at the origin.
fn scale_fallback(positions: &[MotionPoint]) -> Vec<WorldPosition> {
    if positions.is_empty() {
        return Vec::new();
    }
    let (mut min1, mut max1, mut min2, mut max2) = (f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY);
    for p in positions {
        min1 = f64::min(min1, p.raw1);
        max1 = f64::max(max1, p.raw1);
        min2 = f64::min(min2, p.raw2);
        max2 = f64::max(max2, p.raw2);
    }
    let cx = -(min2 + max2) / 2.0 * DEFAULT_SCALE;
    let cy = (min1 + max1) / 2.0 * DEFAULT_SCALE;
    positions
        .iter()
        .map(|p| WorldPosition {
            x: -p.raw2 * DEFAULT_SCALE - cx,
            y: p.raw1 * DEFAULT_SCALE - cy,
            frame: p.frame,
            in_vehicle: p.in_vehicle,
            vehicle: p.vehicle,
        })
        .collect()
}

/// All paths to world coordinates with one transform anchored on humans.
pub fn scale_paths_to_world(
    paths: &[PlayerPath],
    initial_spawns: &[Pad],
    map_bounds: Option<&MapBounds>,
    composite: bool,
    asset_id: Option<&str>,
    coord_layout: Option<i64>,
    level: Option<&LevelBox>,
    c2_width: Option<u32>,
) -> (Vec<Vec<WorldPosition>>, Option<Calibration>) {
    // Bots are in the same units: with no human positions they anchor it.
    let mut humans: Vec<&PlayerPath> = paths.iter().filter(|p| !p.is_bot && !p.positions.is_empty()).collect();
    if humans.is_empty() {
        humans = paths.iter().collect();
    }
    let calib =
        calibrate_from_spawns(&humans, initial_spawns, map_bounds, composite, asset_id, coord_layout, level, c2_width);
    let world = paths
        .iter()
        .map(|p| {
            if p.positions.is_empty() {
                Vec::new()
            } else if let Some(c) = &calib {
                apply_calibration(&p.positions, c)
            } else {
                scale_fallback(&p.positions)
            }
        })
        .collect();
    (world, calib)
}
