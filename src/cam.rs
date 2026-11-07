//! Cutting-path building blocks: cut direction, ramped entry, holding tabs, exact raster
//! clipping, stay-down linking, arc fitting and machine post-processors.

use crate::geom::{self, Poly};
use egui::{pos2, Pos2};
use serde::{Deserialize, Serialize};

/// A move list: (x, y, z) points the tool passes through in order.
pub type Chain = Vec<[f32; 3]>;

// ------------------------------------------------------------------ direction

/// Run each loop so the cut is climb (or conventional) milling with a clockwise spindle.
/// Offset loops come from Clipper2 with the cut-side region on their left; for climb milling
/// the material must be on the tool's right.
pub fn orient_loops(loops: &mut [Poly], material_on_left: bool, climb: bool) {
    for l in loops.iter_mut() {
        // Region on the left = counter-clockwise outer edge / clockwise hole edge.
        let want_reverse = material_on_left == climb;
        if want_reverse {
            l.pts.reverse();
        }
    }
}

// ------------------------------------------------------------------ resampling

fn seg_len(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((b[0] - a[0]).powi(2) + (b[1] - a[1]).powi(2)).sqrt()
}

fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t]
}

/// Split a path so it has a point exactly at each of the given distances along it.
fn split_at(path: &[[f32; 3]], cuts: &[f32]) -> Chain {
    let mut out: Chain = Vec::with_capacity(path.len() + cuts.len());
    let mut cuts: Vec<f32> = cuts.to_vec();
    cuts.sort_by(f32::total_cmp);
    let mut ci = 0;
    let mut s = 0.0;
    if let Some(f) = path.first() {
        out.push(*f);
    }
    for w in path.windows(2) {
        let l = seg_len(w[0], w[1]);
        while ci < cuts.len() && cuts[ci] <= s + l {
            if cuts[ci] > s + 1e-5 && cuts[ci] < s + l - 1e-5 {
                out.push(lerp3(w[0], w[1], (cuts[ci] - s) / l));
            }
            ci += 1;
        }
        out.push(w[1]);
        s += l;
    }
    out
}

pub fn path_length(path: &[[f32; 3]]) -> f32 {
    path.windows(2).map(|w| seg_len(w[0], w[1])).sum()
}

// ------------------------------------------------------------------ ramps

/// One pass round a closed loop at depth `z`, entering on a ramp from `z_from` over `ramp`
/// mm along the path (instead of plunging straight down). The ramped stretch is cut again at
/// full depth at the end so the floor is level everywhere.
pub fn ramped_loop(lp: &[Pos2], z_from: f32, z: f32, ramp: f32) -> Chain {
    let mut closed: Chain = lp.iter().map(|p| [p.x, p.y, z]).collect();
    if let Some(f) = lp.first() {
        closed.push([f.x, f.y, z]);
    }
    let total = path_length(&closed);
    if ramp <= 1e-3 || total <= 1e-3 || z_from <= z + 1e-4 {
        return closed;
    }
    let ramp = ramp.min(total * 0.95);
    let pts = split_at(&closed, &[ramp]);
    let mut out = Chain::new();
    let mut s = 0.0;
    let mut tail = Chain::new();
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            s += seg_len(pts[i - 1], *p);
        }
        if s <= ramp + 1e-4 {
            let t = (s / ramp).min(1.0);
            out.push([p[0], p[1], z_from + (z - z_from) * t]);
            if i > 0 {
                tail.push(*p);
            }
        } else {
            out.push(*p);
        }
    }
    // re-cut the ramped stretch level
    out.extend(tail);
    out
}

// ------------------------------------------------------------------ tabs

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tabs {
    pub on: bool,
    pub count: u32,
    /// Length of each tab along the path, mm.
    pub length: f32,
    /// How thick the tab is (how much material it leaves above the bottom of the cut), mm.
    pub thickness: f32,
}

impl Default for Tabs {
    fn default() -> Self {
        Tabs { on: false, count: 4, length: 8.0, thickness: 3.0 }
    }
}

/// Lift the tool over evenly spaced tabs on a closed-loop chain: wherever the cut would go
/// below `tab_top` inside a tab, it rides at `tab_top` instead (with short ramps either side).
/// `perimeter` is the loop's length; a chain longer than that (a ramped pass that re-cuts its
/// start) keeps the tabs in the same places on its second time round.
pub fn apply_tabs(chain: &Chain, tabs: &Tabs, tab_top: f32, perimeter: f32) -> Chain {
    if !tabs.on || tabs.count == 0 || chain.len() < 2 || perimeter <= 1e-3 {
        return chain.clone();
    }
    let total = path_length(chain);
    if perimeter <= tabs.length * tabs.count as f32 {
        return chain.clone();
    }
    let spacing = perimeter / tabs.count as f32;
    let ramp = (tabs.length * 0.25).min(2.0);
    // tab centres sit half a spacing in, so none lands on the start where the tool enters
    let tabs_at: Vec<(f32, f32)> = (0..tabs.count).map(|k| {
        let c = spacing * (k as f32 + 0.5);
        (c - tabs.length * 0.5, c + tabs.length * 0.5)
    }).collect();
    let mut cuts = Vec::new();
    let mut lap = 0.0;
    while lap < total {
        for &(a, b) in &tabs_at {
            cuts.extend([a - ramp, a, b, b + ramp].map(|c| c + lap));
        }
        lap += perimeter;
    }
    let pts = split_at(chain, &cuts);
    let lift = |s: f32| -> f32 {
        let s = s % perimeter;
        // 0 outside tabs, 1 on top of a tab, linear on the ramps
        tabs_at.iter().map(|&(a, b)| {
            if s >= a && s <= b {
                1.0
            } else if s > a - ramp && s < a {
                (s - (a - ramp)) / ramp
            } else if s > b && s < b + ramp {
                1.0 - (s - b) / ramp
            } else {
                0.0
            }
        }).fold(0.0, f32::max)
    };
    let mut s = 0.0;
    let mut out = Chain::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            s += seg_len(pts[i - 1], *p);
        }
        let k = lift(s);
        let z = if p[2] < tab_top { p[2] + (tab_top - p[2]) * k } else { p[2] };
        out.push([p[0], p[1], z]);
    }
    out
}

// ------------------------------------------------------------------ raster clipping

/// The stretches of the horizontal line at height `y` that lie inside `region` (even-odd),
/// found exactly from where the line crosses each edge.
pub fn scanline(region: &[Poly], y: f32) -> Vec<(f32, f32)> {
    let mut xs: Vec<f32> = Vec::new();
    for poly in region.iter().filter(|p| p.closed) {
        let n = poly.pts.len();
        for i in 0..n {
            let (a, b) = (poly.pts[i], poly.pts[(i + 1) % n]);
            if (a.y > y) != (b.y > y) {
                xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
            }
        }
    }
    xs.sort_by(f32::total_cmp);
    xs.chunks(2).filter(|c| c.len() == 2 && c[1] - c[0] > 1e-4).map(|c| (c[0], c[1])).collect()
}

/// Whether the straight move from `a` to `b` stays inside `region` (so the tool can stay down).
pub fn stays_inside(region: &[Poly], a: Pos2, b: Pos2) -> bool {
    let steps = (((b - a).length() / 0.5).ceil() as usize).clamp(2, 400);
    (0..=steps).all(|k| {
        let p = a + (b - a) * (k as f32 / steps as f32);
        geom::point_inside(p, region) || geom::dist_to_polys(p, region) < 0.01
    })
}

/// Turn a closed loop so it starts at the point nearest `from`.
pub fn start_nearest(lp: &[Pos2], from: Pos2) -> Vec<Pos2> {
    let Some((k, _)) = lp.iter().enumerate().min_by(|a, b| (*a.1 - from).length().total_cmp(&(*b.1 - from).length())) else {
        return lp.to_vec();
    };
    lp[k..].iter().chain(lp[..k].iter()).copied().collect()
}

/// Join consecutive chains at the same depth into one where the link stays inside `region`,
/// so the tool doesn't lift between them.
pub fn link_chains(chains: Vec<Chain>, region: &[Poly]) -> Vec<Chain> {
    let mut out: Vec<Chain> = Vec::new();
    for ch in chains {
        if let (Some(last), Some(first)) = (out.last_mut(), ch.first()) {
            if let Some(end) = last.last() {
                let same_z = (end[2] - first[2]).abs() < 1e-4;
                if same_z && stays_inside(region, pos2(end[0], end[1]), pos2(first[0], first[1])) {
                    last.extend(ch);
                    continue;
                }
            }
        }
        out.push(ch);
    }
    out
}

// ------------------------------------------------------------------ arc fitting

/// One G-code move.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Move {
    Line([f32; 3]),
    /// Circular move in the XY plane to `end`, around `centre`, clockwise or not.
    Arc { end: [f32; 3], centre: Pos2, cw: bool },
}

fn circle3(a: Pos2, b: Pos2, c: Pos2) -> Option<(Pos2, f32)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-9 {
        return None;
    }
    let (a2, b2, c2) = (a.to_vec2().length_sq(), b.to_vec2().length_sq(), c.to_vec2().length_sq());
    let ux = (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d;
    let uy = (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d;
    let centre = pos2(ux, uy);
    Some((centre, (a - centre).length()))
}

/// Replace runs of short level segments that follow a circle (within `tol` mm) with arcs,
/// which machines cut smoother and which keeps the file small. Needs at least 4 points per arc.
pub fn fit_arcs(chain: &[[f32; 3]], tol: f32) -> Vec<Move> {
    let p = |i: usize| pos2(chain[i][0], chain[i][1]);
    let turn = |i: usize| {
        let (a, b, c) = (p(i - 1), p(i), p(i + 1));
        (b - a).x * (c - b).y - (b - a).y * (c - b).x
    };
    let mut out = Vec::new();
    let n = chain.len();
    let mut i = 0;
    while i + 1 < n {
        let mut best: Option<(usize, Pos2, bool)> = None;
        let mut j = i + 3;
        while j < n {
            if (chain[j][2] - chain[i][2]).abs() > 1e-4 || (chain[j - 1][2] - chain[i][2]).abs() > 1e-4 {
                break;
            }
            let Some((c, r)) = circle3(p(i), p((i + j) / 2), p(j)) else { break };
            if r > 2000.0 || r < 0.01 {
                break;
            }
            // every point on the circle, and every point turning the same way
            let on_circle = (i..=j).all(|k| ((p(k) - c).length() - r).abs() <= tol);
            let t0 = turn(i + 1);
            let same_turn = t0.abs() > 1e-9 && (i + 1..j).all(|k| turn(k) * t0 > 0.0);
            // no single step sweeping more than 90° (a chord that big isn't on this arc)
            let small_steps = (i..j).all(|k| (p(k + 1) - p(k)).length() < r * 1.4);
            if !(on_circle && same_turn && small_steps) {
                break;
            }
            best = Some((j, c, t0 < 0.0));
            j += 1;
        }
        match best {
            Some((j, centre, cw)) => {
                out.push(Move::Arc { end: chain[j], centre, cw });
                i = j;
            }
            None => {
                out.push(Move::Line(chain[i + 1]));
                i += 1;
            }
        }
    }
    out
}

// ------------------------------------------------------------------ post-processors

/// Which machine controller the G-code is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Post {
    #[default]
    GrblMm,
    GrblInch,
    Mach3Mm,
    Mach3Inch,
    LinuxCncMm,
}

impl Post {
    pub const ALL: [Post; 5] = [Post::GrblMm, Post::GrblInch, Post::Mach3Mm, Post::Mach3Inch, Post::LinuxCncMm];
    pub fn label(self) -> &'static str {
        match self {
            Post::GrblMm => "GRBL (mm)",
            Post::GrblInch => "GRBL (inch)",
            Post::Mach3Mm => "Mach3 / UCCNC (mm)",
            Post::Mach3Inch => "Mach3 / UCCNC (inch)",
            Post::LinuxCncMm => "LinuxCNC (mm)",
        }
    }
    pub fn inch(self) -> bool {
        matches!(self, Post::GrblInch | Post::Mach3Inch)
    }
    /// GRBL has no tool changer: it pauses (M0) for a manual change instead of M6.
    pub fn tool_change(self) -> bool {
        !matches!(self, Post::GrblMm | Post::GrblInch)
    }
    pub fn extension(self) -> &'static str {
        match self {
            Post::GrblMm | Post::GrblInch => "nc",
            Post::Mach3Mm | Post::Mach3Inch => "tap",
            Post::LinuxCncMm => "ngc",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(s: f32) -> Vec<Pos2> {
        vec![pos2(0.0, 0.0), pos2(s, 0.0), pos2(s, s), pos2(0.0, s)]
    }

    #[test]
    fn ramp_enters_gradually_then_recuts_level() {
        let ch = ramped_loop(&square(40.0), 0.0, -3.0, 20.0);
        assert!((ch[0][2] - 0.0).abs() < 1e-4, "starts at the top of the pass");
        let at20 = ch.iter().find(|p| (p[0] - 20.0).abs() < 1e-3 && p[1].abs() < 1e-3).unwrap();
        assert!((at20[2] + 3.0).abs() < 1e-3, "reaches full depth after 20 mm");
        assert!((ch.last().unwrap()[2] + 3.0).abs() < 1e-4, "finishes level at full depth");
        assert!(path_length(&ch) > 160.0 + 19.0, "goes round once plus the ramped stretch");
    }

    #[test]
    fn tabs_lift_only_inside_tab_spans() {
        let mut ch: Chain = square(100.0).iter().map(|p| [p.x, p.y, -10.0]).collect();
        ch.push([0.0, 0.0, -10.0]);
        let tabs = Tabs { on: true, count: 4, length: 10.0, thickness: 3.0 };
        let out = apply_tabs(&ch, &tabs, -7.0, 400.0);
        let lifted = out.iter().filter(|p| (p[2] + 7.0).abs() < 1e-3).count();
        assert!(lifted >= 8, "two points on top of each of 4 tabs, got {lifted}");
        assert!(out.iter().all(|p| p[2] >= -10.0 - 1e-4 && p[2] <= -7.0 + 1e-4));
        // the first tab is centred half a spacing in (50 mm), so it spans 45–55 mm on side one
        for x in [45.0, 55.0] {
            let p = out.iter().find(|p| (p[0] - x).abs() < 1e-3 && p[1].abs() < 1e-3).expect("tab edge point");
            assert!((p[2] + 7.0).abs() < 1e-3, "tab edge at {x} mm is on top of the tab");
        }
        let before = out.iter().find(|p| (p[0] - 43.0).abs() < 1e-3 && p[1].abs() < 1e-3).expect("ramp start");
        assert!((before[2] + 10.0).abs() < 1e-3, "full depth before the tab's ramp");
    }

    #[test]
    fn scanline_is_exact() {
        let r = vec![Poly { pts: square(10.0), closed: true }];
        assert_eq!(scanline(&r, 5.0), vec![(0.0, 10.0)]);
    }

    #[test]
    fn arcs_replace_circle_points() {
        // a 10 mm radius quarter circle in 30 steps, then a straight line
        let mut ch: Chain = (0..=30).map(|k| {
            let a = k as f32 / 30.0 * std::f32::consts::FRAC_PI_2;
            [10.0 * a.cos(), 10.0 * a.sin(), -1.0]
        }).collect();
        ch.push([-20.0, 10.0, -1.0]);
        let moves = fit_arcs(&ch, 0.005);
        assert_eq!(moves.len(), 2, "{moves:?}");
        match moves[0] {
            Move::Arc { end, centre, cw } => {
                assert!(!cw, "counter-clockwise quarter");
                assert!(centre.to_vec2().length() < 0.01);
                assert!((end[1] - 10.0).abs() < 1e-3);
            }
            _ => panic!("expected an arc"),
        }
    }

    #[test]
    fn climb_reverses_outside_loops() {
        let mut l = vec![Poly { pts: square(10.0), closed: true }];
        orient_loops(&mut l, true, true);
        assert!(geom::signed_area(&l[0].pts) < 0.0, "outside + climb runs clockwise");
    }
}
