//! Our own placement of NM's parts on a model: Homer's proportions kept, laid on the model's bones, as thick as its
//! mesh.
//!
//! From the plain fit (the model's joints, no size or placement edits: Homer's parts scaled by the model's bone
//! lengths), each part
//! - keeps Homer's length for its bone (his capsule's half length and its caps' reach past the joints, scaled);
//! - lies along the model's bone when it is long (a limb's, a clavicle's along the joint to the next), its
//!   cross-section centred in its mesh; a foot's or hand's box along its own bones (a foot's level) and over its mesh,
//!   a foot's sole on the mesh's; the head in the middle of its mesh; the pelvis and the spine keep their way and place;
//! - is as thick as Homer's times one factor for the whole body, a bounded factor for its group (legs, arms, torso,
//!   pelvis, neck, head) and a small one of its own (a limb's), all from the mesh.
//!
//! It measures the model's ragdoll hulls (its `.phy`) where a part has one, else its mesh (a vertex is the part's whose
//! bones carry most of it), robustly: a limb's cross-section is its inner shell round the bone (holsters, pouches and
//! coats lie outside it), the measures are medians with the outliers past three deviations left out, a cross-section
//! must lie round its bone to count, and the two sides of a pair are one. What isn't measured well stays as the plain
//! fit has it, and the status line says why (docs/rig-editor.md).
//!
//! The result is the rig's edits (sizes, the parts moved and turned on their bones); masses stay.

use delirium::prelude::*;
use dms_fit::M;
use dms_fit::fit::Fit;
use dms_fit::rigs::{self, PARTS};
use dms_fit::skeleton::Skeleton;
use dms_game::body::Shape;

use crate::Homer;
use crate::mesh::Mesh;
use crate::rigfile::Rig;
use crate::scene;

/// The joint each part runs to (its segment); `None`: a leaf or the pelvis.
const NEXT: [Option<usize>; 21] = [
    None,
    Some(2),
    Some(3),
    None,
    Some(5),
    Some(6),
    None,
    Some(8),
    Some(9),
    Some(10),
    Some(11),
    Some(12),
    None,
    Some(14),
    Some(15),
    Some(16),
    None,
    Some(18),
    Some(19),
    Some(20),
    None,
];

/// The limbs measured round their bones (their thickness and centre): the thighs, shins, upper and lower arms.
const LIMBS: [usize; 8] = [1, 2, 4, 5, 14, 15, 18, 19];
const SPINE: [usize; 4] = [7, 8, 9, 10];
const LEAVES: [usize; 4] = [3, 6, 16, 20];
const FEET: [usize; 2] = [3, 6];
/// The pairs, left and right.
const PAIRS: [(usize, usize); 7] = [(1, 4), (2, 5), (3, 6), (13, 17), (14, 18), (15, 19), (16, 20)];
/// A vertex counts for a part when the part's bones carry this much of it (firm; the blends round the joints left out
/// where there are enough without them), and at all when they carry most of it.
const FIRM: f32 = 0.75;
/// The share of a segment's middle that measures it (the ends are its joints' blends).
const MIDDLE: (f32, f32) = (0.2, 0.8);
/// The fewest points a measure takes, and the sectors of the eight round a bone its cross-section must reach to be
/// centred on (a mesh on one side of its bone, a clavicle's or a sleeve's edge, says nothing of the middle).
const FEWEST: usize = 12;
const SECTORS: usize = 6;
/// The farthest a vertex is from its bone's line and still measures it, to the plain fit's thickness.
const REACH: f32 = 3.0;
/// The outliers: past the median by this many deviations (the median's absolute deviation, at least a tenth of it).
const DEVIATIONS: f32 = 3.0;
/// The body's factor's bounds; a group's to the body's; a part's own to its group's (thinner rather than fatter: a
/// woman's shins, a thin man's forearms are thinner to their thighs and upper arms than Homer's).
const BODY: (f32, f32) = (0.5, 1.8);
const GROUP: (f32, f32) = (0.75, 1.33);
const OWN: (f32, f32) = (0.7, 1.15);
/// The head's, the neck's, the hands' and the feet's factors' bounds, Homer's (not the body's: armour and coats make a
/// body thick, not its head, hands and boots).
const ENDS: (f32, f32) = (0.6, 1.4);
/// The thinnest any part is, to Homer's.
pub const FLOOR: f32 = 0.35;
/// The sides of a pair agree when their own factors are this close (a share), their centres this close (to the
/// thickness).
const AGREE: (f32, f32) = (0.2, 0.35);
/// The farthest a limb's centre moves off its bone, to its thickness; the head off the plain fit's, to its reach.
const OFFSET: f32 = 0.5;
const HEAD_MOVE: f32 = 1.0;
/// A foot's or hand's length to Homer's: the mesh's, within these.
const LENGTH: (f32, f32) = (0.8, 1.25);
/// The most a foot's or hand's way from its bones may be off its mesh's (cos 35°); past it, the mesh's.
const LEAF_AGREE: f32 = 0.82;
/// A foot's mesh is long enough to lie along when it spreads this much more along its axis than across (a foot's
/// footprint about 6, a boot's ring about 1).
const LONG_MESH: f32 = 2.0;
/// The most a long part is turned to lie along its bone (cos 50°).
const ALIGN: f32 = 0.64;

/// Where a part's shape goes: its centre, its shape's frame (the shape's own axes), its shape with the new sizes.
pub struct Target {
    pub k: usize,
    pub centre: Vec3,
    pub frame: Quat,
    pub shape: Shape,
}

/// The thickness factors found (the mesh's to Homer's scaled), for the status line.
#[derive(Clone, Copy)]
pub struct Factors {
    pub body: f32,
    pub legs: f32,
    pub arms: f32,
    pub torso: f32,
    pub pelvis: f32,
    pub neck: f32,
    pub head: f32,
}

pub fn shape_rot(s: &Shape) -> Quat {
    match *s {
        Shape::Capsule { rot, .. } | Shape::Box { rot, .. } => rot,
    }
}

/// The shape's long axis in its own frame (a capsule's Y, a box's longest) and whether it has one worth lying along.
fn long_axis(s: &Shape) -> (Vec3, bool) {
    match *s {
        Shape::Capsule { radius, half, .. } => (Vec3::Y, half >= radius * 0.75),
        Shape::Box { half, .. } => {
            let i = (0..3).max_by(|&a, &b| half[a].total_cmp(&half[b])).unwrap_or(1);
            let mut h = half.to_array();
            h.sort_by(f32::total_cmp);
            (Vec3::AXES[i], h[2] >= h[1] * 1.4)
        }
    }
}

/// The `q` quantile of `v` (sorted in place).
fn quantile(v: &mut [f32], q: f32) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    v.sort_by(f32::total_cmp);
    v[((v.len() - 1) as f32 * q).round() as usize]
}

/// The median (the two middle values' mean for an even count: two measures' median is neither of them).
fn median(mut v: Vec<f32>) -> Option<f32> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let n = v.len();
    Some(if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) * 0.5 })
}

/// Each axis' median.
fn median3(pts: &[Vec3]) -> Vec3 {
    Vec3::from_array([0, 1, 2].map(|i| quantile(&mut pts.iter().map(|o| o[i]).collect::<Vec<_>>(), 0.5)))
}

/// `v` without its outliers (past the median by [`DEVIATIONS`] median absolute deviations).
fn inliers(v: &[f32]) -> Vec<bool> {
    let Some(m) = median(v.to_vec()) else { return vec![] };
    let mad = median(v.iter().map(|x| (x - m).abs()).collect()).unwrap_or(0.0).max(m.abs() * 0.1).max(1e-4);
    v.iter().map(|x| (x - m).abs() <= DEVIATIONS * mad).collect()
}

fn seg_dist(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(1e-9)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

fn thickness(s: &Shape) -> f32 {
    match *s {
        Shape::Capsule { radius, .. } => radius,
        Shape::Box { half, .. } => half.min_element(),
    }
}

/// A part's vertices (m): the firm ones and all its own.
#[derive(Default, Clone)]
struct Cloud {
    firm: Vec<Vec3>,
    all: Vec<Vec3>,
}

impl Cloud {
    /// The firm points `keep` takes when there are enough of them, else all its points it takes.
    fn pick(&self, keep: impl Fn(&Vec3) -> bool) -> Vec<Vec3> {
        let firm: Vec<Vec3> = self.firm.iter().copied().filter(|p| keep(p)).collect();
        if firm.len() >= FEWEST { firm } else { self.all.iter().copied().filter(|p| keep(p)).collect() }
    }
}

/// Each bone's part: its own mapped bone's, else its nearest mapped ancestor's.
fn bone_parts(skel: &Skeleton, map: &[Option<String>; 21]) -> Vec<Option<usize>> {
    let own: Vec<Option<usize>> = skel.bones.iter().map(|b| map.iter().position(|x| x.as_deref() == Some(b.name.as_str()))).collect();
    (0..skel.bones.len())
        .map(|mut b| loop {
            if let Some(k) = own[b] {
                break Some(k);
            }
            match skel.bones[b].parent {
                Some(p) => b = p,
                None => break None,
            }
        })
        .collect()
}

/// The drawn vertices (m) by part: a vertex is the part's whose bones carry most of its weight (a twist bone's share
/// adds to its limb's), firm where they carry [`FIRM`]; the spine's by the nearest of its four segments.
fn clouds(skel: &Skeleton, mesh: &Mesh, map: &[Option<String>; 21], joints: &[Vec3]) -> Vec<Cloud> {
    let part_of = bone_parts(skel, map);
    // Only the vertices drawn: the `.vvd` also holds the other body groups' (a model's held props, its other heads).
    let mut drawn = vec![false; mesh.pos.len()];
    for &i in mesh.tris.iter().flatten() {
        if let Some(d) = drawn.get_mut(i as usize) {
            *d = true;
        }
    }
    let mut out = vec![Cloud::default(); 21];
    for ((p, bones), _) in mesh.pos.iter().zip(&mesh.bones).zip(&drawn).filter(|x| *x.1) {
        let mut weight = [0.0f32; 21];
        let mut total = 0.0;
        for &(b, w) in bones.iter().filter(|x| x.1 > 0.0) {
            total += w;
            if let Some(k) = part_of.get(b as usize).copied().flatten() {
                weight[k] += w;
            }
        }
        let Some((mut k, w)) = weight.iter().copied().enumerate().max_by(|a, b| a.1.total_cmp(&b.1)) else { continue };
        let w = w / total.max(1e-6);
        if w < 0.5 {
            continue;
        }
        let p = Vec3::from_array(*p) * M;
        if SPINE.contains(&k) {
            k = SPINE.into_iter().min_by(|&a, &b| seg_dist(p, joints[a], joints[a + 1]).total_cmp(&seg_dist(p, joints[b], joints[b + 1]))).unwrap_or(k);
        }
        out[k].all.push(p);
        if w >= FIRM {
            out[k].firm.push(p);
        }
    }
    out
}

/// The model's ragdoll hulls (its `.phy`: one convex solid a physics bone, made simple, no holsters, coats or
/// fingers) as points over their surfaces (m, about a centimetre apart) by part, as [`clouds`] has the mesh's; `None`
/// when it can't be read.
fn hull_clouds(skel: &Skeleton, phy: &[u8], map: &[Option<String>; 21], joints: &[Vec3]) -> Option<Vec<Cloud>> {
    let file = dms_vphysics::phy::read(phy).ok()?;
    let part_of = bone_parts(skel, map);
    let mut out = vec![Cloud::default(); 21];
    for b in dms_vphysics::phy::blocks(&file.text).iter().filter(|b| b.name == "solid") {
        let (Some(i), Some(name)) = (b.get("index").and_then(|i| i.parse::<usize>().ok()), b.get("name")) else { continue };
        let (Some(solid), Some(bone)) = (file.solids.get(i), skel.find(name)) else { continue };
        let Some(mut k) = part_of[bone] else { continue };
        // The solid in its bone's frame (m), the bone where the mesh is bound (`poseToBone`).
        let (at, rot) = skel.bones[bone].pose;
        let world = |p: Vec3| at * M + rot * p;
        for t in &solid.triangles {
            let [a, b, c] = t.map(|i| world(solid.points[i]));
            let n = ((a.distance(b).max(b.distance(c)).max(c.distance(a)) / 0.01).ceil() as usize).clamp(1, 40);
            for u in 0..=n {
                for v in 0..=n - u {
                    let p = a + (b - a) * (u as f32 / n as f32) + (c - a) * (v as f32 / n as f32);
                    if SPINE.contains(&k) {
                        k = SPINE.into_iter().min_by(|&x, &y| seg_dist(p, joints[x], joints[x + 1]).total_cmp(&seg_dist(p, joints[y], joints[y + 1]))).unwrap_or(k);
                    }
                    out[k].firm.push(p);
                    out[k].all.push(p);
                }
            }
        }
    }
    Some(out)
}

/// Whether a part's hull points (`hull`) lie on its mesh's (`mesh`): their middles within half the mesh's spread (and
/// 3 cm), their spreads (the median distance from the middle) within [`HULL_FITS`] of each other. No mesh to check
/// against: the hull's.
fn hull_fits(hull: &[Vec3], mesh: &[Vec3]) -> bool {
    if mesh.len() < FEWEST {
        return true;
    }
    let (ch, cm) = (median3(hull), median3(mesh));
    let spread = |p: &[Vec3], c: Vec3| median(p.iter().map(|x| x.distance(c)).collect()).unwrap_or(0.0);
    let (sh, sm) = (spread(hull, ch), spread(mesh, cm));
    ch.distance(cm) <= sm * 0.5 + 0.03 && (HULL_FITS.0..=HULL_FITS.1).contains(&(sh / sm.max(1e-3)))
}

/// A hull's spread to its mesh's that still fits it: thinner (clothes and gear make a mesh fat), hardly fatter (a
/// hull fatter than its mesh is another model's).
const HULL_FITS: (f32, f32) = (0.6, 1.2);

/// A cross-section round a bone: its centre (an offset across the bone's line), its radius, and whether it lies all
/// round the bone (else its centre says nothing).
#[derive(Clone, Copy)]
pub struct Section {
    pub centre: Vec3,
    pub radius: f32,
    pub round: bool,
}

/// The circle through points in a plane (least squares, Kåsa's): its centre and radius; `None` when they're on a line.
fn circle(pts: &[Vec2]) -> Option<(Vec2, f32)> {
    let n = pts.len() as f64;
    let m = pts.iter().fold([0.0f64; 2], |s, p| [s[0] + f64::from(p.x) / n, s[1] + f64::from(p.y) / n]);
    // x² + y² + a x + b y + c = 0 about the points' mean (better conditioned): the normal equations, by Cramer.
    let (mut ata, mut atb) = ([[0.0f64; 3]; 3], [0.0f64; 3]);
    for p in pts {
        let row = [f64::from(p.x) - m[0], f64::from(p.y) - m[1], 1.0];
        let rhs = -(row[0] * row[0] + row[1] * row[1]);
        for i in 0..3 {
            for j in 0..3 {
                ata[i][j] += row[i] * row[j];
            }
            atb[i] += row[i] * rhs;
        }
    }
    let det = |a: [[f64; 3]; 3]| {
        a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1]) - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0]) + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0])
    };
    let d = det(ata);
    if d.abs() < 1e-18 {
        return None;
    }
    let s: [f64; 3] = std::array::from_fn(|c| {
        let mut a = ata;
        for (r, row) in a.iter_mut().enumerate() {
            row[c] = atb[r];
        }
        det(a) / d
    });
    let (cx, cy) = (-s[0] * 0.5, -s[1] * 0.5);
    let r2 = cx * cx + cy * cy - s[2];
    (r2 > 0.0 && cx.is_finite() && cy.is_finite()).then(|| (Vec2::new((cx + m[0]) as f32, (cy + m[1]) as f32), r2.sqrt() as f32))
}

/// The cross-section of `pts` (offsets across the bone's line `d`, its outliers left out): the circle through them,
/// its radius their median distance from its centre, and whether they lie all round the bone.
pub fn ring(pts: &[Vec3], d: Vec3) -> Option<Section> {
    if pts.len() < FEWEST {
        return None;
    }
    let u = d.any_orthonormal_vector();
    let v = d.cross(u);
    let flat: Vec<Vec2> = pts.iter().map(|o| Vec2::new(o.dot(u), o.dot(v))).collect();
    // The limb is the inner shell round its bone: what's on it (a holster, pouches, a coat's tails), however dense,
    // lies farther out. Each sector round the bone keeps its points near its nearest (its 5% quantile: a stray
    // inside left out).
    let sector = |p: &Vec2| ((p.y.atan2(p.x) / std::f32::consts::TAU + 1.0) * SHELL as f32) as usize % SHELL;
    let mut near = [f32::MAX; SHELL];
    for (s, n) in near.iter_mut().enumerate() {
        let mut d: Vec<f32> = flat.iter().filter(|p| sector(p) == s).map(|p| p.length()).collect();
        if !d.is_empty() {
            *n = quantile(&mut d, 0.05);
        }
    }
    let shell: Vec<Vec2> = flat.iter().copied().filter(|p| p.length() <= near[sector(p)] * 1.3 + 0.005).collect();
    let (c, radius, round) = ring_fit(shell)?;
    Some(Section { centre: u * c.x + v * c.y, radius, round })
}

/// The sectors round a bone its mesh's inner shell is found in.
const SHELL: usize = 16;

/// The circle through a ring's points, the points far off it left out till they're the circle's: its centre, its
/// radius (the points' median distance) and whether it lies round the bone.
fn ring_fit(mut flat: Vec<Vec2>) -> Option<(Vec2, f32, bool)> {
    if flat.len() < FEWEST {
        return None;
    }
    let mut fit = circle(&flat)?;
    for _ in 0..4 {
        let off: Vec<f32> = flat.iter().map(|p| (p.distance(fit.0) - fit.1).abs()).collect();
        let m = median(off.clone()).unwrap_or(0.0);
        let mad = median(off.iter().map(|x| (x - m).abs()).collect()).unwrap_or(0.0).max(fit.1 * 0.05);
        let keep: Vec<Vec2> = flat.iter().zip(&off).filter(|x| *x.1 <= m + DEVIATIONS * mad).map(|x| *x.0).collect();
        if keep.len() < FEWEST {
            return None;
        }
        flat = keep;
        fit = circle(&flat)?;
    }
    let radius = median(flat.iter().map(|p| p.distance(fit.0)).collect())?;
    // Round the bone: the sectors its points reach round the circle's centre, the bone within it.
    let mut seen = [false; 8];
    for p in &flat {
        let o = *p - fit.0;
        let s = ((o.y.atan2(o.x) / std::f32::consts::TAU + 1.0) * 8.0) as usize % 8;
        seen[s] = true;
    }
    // A circle far off its points' middle is a fit to an arc: no centre to trust.
    let round = seen.iter().filter(|x| **x).count() >= SECTORS && fit.1 < radius * 1.5 && fit.0.length() < radius;
    Some((fit.0, radius, round))
}

/// The cross-section of `cloud` in the middle of the segment `a`–`b`, its points within `reach` of the line.
fn section(cloud: &Cloud, a: Vec3, b: Vec3, reach: f32) -> Option<Section> {
    let (d, len) = ((b - a).normalize_or_zero(), a.distance(b));
    if d == Vec3::ZERO {
        return None;
    }
    let across = |p: &Vec3| *p - a - d * (*p - a).dot(d);
    let pts: Vec<Vec3> = cloud
        .pick(|p| {
            let t = (*p - a).dot(d);
            t >= len * MIDDLE.0 && t <= len * MIDDLE.1 && across(p).length() <= reach
        })
        .iter()
        .map(across)
        .collect();
    ring(&pts, d)
}

/// How far `cloud`'s points (the shape's span along its axis `axis` through `c`) lie from that axis, robustly; with
/// `depth`, only along it (a spine capsule lies across the body: its radius is the body's depth).
fn spread(cloud: &Cloud, c: Vec3, axis: Vec3, half: f32, depth: Option<Vec3>) -> Option<f32> {
    let d: Vec<f32> = cloud
        .pick(|p| (*p - c).dot(axis).abs() <= half.max(0.02))
        .iter()
        .map(|p| {
            let o = *p - c - axis * (*p - c).dot(axis);
            depth.map_or(o.length(), |n| o.dot(n).abs())
        })
        .collect();
    if d.len() < FEWEST {
        return None;
    }
    let keep = inliers(&d);
    median(d.iter().zip(keep).filter(|x| x.1).map(|x| *x.0).collect())
}

/// The model's way from a leaf part's bone to its farthest bone below that carries any of the mesh (a foot's toes, a
/// hand's fingers; not an attachment's), else its farthest not named an attachment, in the mesh's bind; `None`
/// without.
fn leaf_way(skel: &Skeleton, mesh: &Mesh, map: &[Option<String>; 21], k: usize) -> Option<Vec3> {
    let b = skel.bones.iter().position(|x| map[k].as_deref() == Some(x.name.as_str()))?;
    let mut carries = vec![false; skel.bones.len()];
    for &(i, w) in mesh.bones.iter().flatten() {
        if w > 0.0
            && let Some(c) = carries.get_mut(i as usize)
        {
            *c = true;
        }
    }
    let below = |mut c: usize| loop {
        match skel.bones[c].parent {
            Some(p) if p == b => break true,
            Some(p) => c = p,
            None => break false,
        }
    };
    let at = skel.bones[b].pose.0;
    let farthest = |keep: &dyn Fn(usize) -> bool| {
        (0..skel.bones.len())
            .filter(|&c| below(c) && keep(c))
            .map(|c| (skel.bones[c].pose.0 - at) * M)
            .max_by(|x, y| x.length().total_cmp(&y.length()))
            .filter(|w| w.length() > 0.5 * M)
    };
    farthest(&|c| carries[c]).or_else(|| farthest(&|c| !skel.bones[c].name.to_ascii_lowercase().contains("attach")))
}

/// The head's shape frame as Homer's head stands (his stance faces the model's way): the fit turns the head with its
/// neck, and a model's neck leaning further forward than Homer's (HL2's citizens 33°, his 15°) bows it.
fn upright_head(plain: &Fit) -> Quat {
    (plain.frames[12].rotation * shape_rot(&plain.body.parts[12].shape)).normalize()
}

/// A head's points' box in `frame`: its middle and half extents (each axis' 3–97% span). Spans, not distances from a
/// median: a face's vertices are many, its back's few, and a median on the face would leave the back of the skull out.
fn head_box(cloud: &Cloud, frame: Quat) -> Option<(Vec3, Vec3)> {
    let pts = cloud.pick(|_| true);
    if pts.len() < FEWEST {
        return None;
    }
    let (mut mid, mut half) = (Vec3::ZERO, Vec3::ZERO);
    for i in 0..3 {
        let axis = frame * Vec3::AXES[i];
        let mut v: Vec<f32> = pts.iter().map(|p| p.dot(axis)).collect();
        let (lo, hi) = (quantile(&mut v, 0.03), quantile(&mut v, 0.97));
        mid += axis * (lo + hi) * 0.5;
        half[i] = (hi - lo) * 0.5;
    }
    Some((mid, half))
}

/// Robust per-part measures and the factors made of them, and what was left out and why.
struct Measured {
    factors: Factors,
    /// Each part's thickness factor to the plain fit's.
    thick: [f32; 21],
    /// The limbs' centres across their bones (offsets), where measured well.
    centre: [Option<Vec3>; 21],
    notes: Vec<String>,
}

/// The middle of `xs` (the median of those near it), `None` without any.
fn robust_middle(xs: &[f32]) -> Option<f32> {
    let keep = inliers(xs);
    median(xs.iter().zip(keep).filter(|x| x.1).map(|x| *x.0).collect())
}

fn measure(clouds: &[Cloud], plain: &Fit) -> Measured {
    let j = &plain.shown_joints;
    let mut notes = vec![];
    let shape = |k: usize| plain.body.parts[k].shape;
    let world_axis = |k: usize| plain.shown[k].rotation * shape_rot(&shape(k)) * long_axis(&shape(k)).0;
    // The limbs' cross-sections, each against its plain thickness.
    let sections: [Option<Section>; 21] = std::array::from_fn(|k| {
        let n = NEXT[k].filter(|_| LIMBS.contains(&k))?;
        section(&clouds[k], j[k], j[n], thickness(&shape(k)) * REACH)
    });
    // Only a cross-section all round its bone says how thick the limb is (an arc's circle is anyone's guess).
    let mut own = [None::<f32>; 21];
    for k in LIMBS {
        own[k] = sections[k].filter(|s| s.round).map(|s| s.radius / thickness(&shape(k)).max(1e-3));
    }
    // The torso's depth (front to back) in each spine segment's slab to its capsule's radius: all the torso's
    // vertices (the clavicles' are the chest's top), a segment's own are few where the spine's bones bunch.
    let mut chest = Cloud::default();
    for k in SPINE.into_iter().chain([13, 17]) {
        chest.firm.extend(&clouds[k].firm);
        chest.all.extend(&clouds[k].all);
    }
    for k in SPINE {
        let s = shape(k);
        let (axis, c) = (world_axis(k), plain.shown[k].translation);
        let (up, len) = ((j[k + 1] - j[k]).normalize_or_zero(), j[k].distance(j[k + 1]));
        let depth = axis.cross(up).normalize_or_zero();
        if depth == Vec3::ZERO {
            continue;
        }
        let half = match s {
            Shape::Capsule { half, .. } => half,
            Shape::Box { half, .. } => half.max_element(),
        };
        let slab = |p: &Vec3| (0.0..=len).contains(&(*p - j[k]).dot(up));
        let cloud = Cloud { firm: chest.firm.iter().copied().filter(slab).collect(), all: chest.all.iter().copied().filter(slab).collect() };
        own[k] = spread(&cloud, c, axis, half, Some(depth)).map(|r| r / thickness(&s).max(1e-3));
    }
    // The pelvis round its capsule's axis; the neck round its bone; the head round its middle.
    if let Shape::Capsule { radius, half, .. } = shape(0) {
        own[0] = spread(&clouds[0], plain.shown[0].translation, world_axis(0), half, None).map(|r| r / radius.max(1e-3));
    }
    own[11] = section(&clouds[11], j[11], j[12], thickness(&shape(11)) * REACH).filter(|s| s.round).map(|s| s.radius / thickness(&shape(11)).max(1e-3));
    // The head: its mesh's extents along the upright head's axes to the shape's (the middle of the three: a hat or a
    // helmet makes one of them).
    if let Some((_, half)) = head_box(&clouds[12], upright_head(plain)) {
        let ext = match shape(12) {
            Shape::Capsule { radius, half, .. } => Vec3::new(radius, radius + half, radius),
            Shape::Box { half, .. } => half,
        };
        own[12] = median((0..3).map(|i| half[i] / ext[i].max(1e-3)).collect());
    }
    // The body's factor: the middle of every measure but the head's (a hat, a helmet, a headcrab).
    let all: Vec<f32> = (0..21).filter(|&k| k != 12).filter_map(|k| own[k]).collect();
    let body = match robust_middle(&all) {
        Some(b) if all.len() >= 4 => {
            if !(BODY.0..=BODY.1).contains(&b) {
                notes.push(format!("the mesh is ×{b:.2} Homer's: held to ×{:.2}", b.clamp(BODY.0, BODY.1)));
            }
            b.clamp(BODY.0, BODY.1)
        }
        _ => {
            notes.push("too little mesh on the bones to measure: Homer's thickness".into());
            1.0
        }
    };
    // A group's factor within its bounds (`of` the body's, else Homer's).
    let mut group = |name: &str, ks: &[usize], bounds: (f32, f32), of: f32, or: f32| {
        let v: Vec<f32> = ks.iter().filter_map(|&k| own[k]).collect();
        match robust_middle(&v) {
            Some(g) => {
                let held = g.clamp(of * bounds.0, of * bounds.1);
                if (held - g).abs() > 0.01 {
                    let whose = if of == 1.0 { "Homer's".to_string() } else { format!("the body's ×{of:.2}") };
                    notes.push(format!("{name} ×{g:.2} held to ×{held:.2} ({whose})"));
                }
                held
            }
            None => {
                notes.push(format!("{name}: no mesh to measure, ×{or:.2}"));
                or
            }
        }
    };
    let legs = group("legs", &[1, 2, 4, 5], GROUP, body, body);
    let arms = group("arms", &[14, 15, 18, 19], GROUP, body, body);
    let torso = group("torso", &SPINE, GROUP, body, body);
    let pelvis = group("pelvis", &[0], GROUP, body, (legs + torso) * 0.5);
    // A thin body's (a skeleton's, a burnt corpse's) head and neck may be as thin as its body lets its groups be.
    let ends = (ENDS.0.min(body * GROUP.0), ENDS.1);
    let head = group("head", &[12], ends, 1.0, 1.0);
    let neck = group("neck", &[11], ends, 1.0, head);
    let factors = Factors { body, legs, arms, torso, pelvis, neck, head };
    let group_of = |k: usize| match k {
        0 => pelvis,
        1..=6 => legs,
        11 => neck,
        12 => head,
        // The clavicles lie between the chest and the arms: the thinner's (straps and pouches make either thick).
        13 | 17 => arms.min(torso),
        14..=20 => arms,
        _ => torso,
    };
    let mut thick: [f32; 21] = std::array::from_fn(group_of);
    // The spine's segments their own within [`OWN`] of the torso's: Homer's chest is fatter to his belly than most
    // models'.
    // A segment without enough mesh of its own (the spine's bones bunched) is its measured neighbours' middle.
    for k in SPINE {
        let near = || {
            let v: Vec<f32> = [k - 1, k + 1].into_iter().filter(|n| SPINE.contains(n)).filter_map(|n| own[n]).collect();
            (!v.is_empty()).then(|| v.iter().sum::<f32>() / v.len() as f32)
        };
        if let Some(x) = own[k].or_else(near) {
            thick[k] = torso * (x / torso).clamp(OWN.0, OWN.1);
        }
    }
    let mut centre = [None::<Vec3>; 21];
    // A limb's own factor and centre, both sides' as one: their mirror images must agree.
    let n = Vec3::new(j[1].x - j[4].x, j[1].y - j[4].y, 0.0).normalize_or_zero();
    let flip = |v: Vec3| v - 2.0 * n * n.dot(v);
    for (a, b) in PAIRS.into_iter().filter(|p| LIMBS.contains(&p.0)) {
        let g = group_of(a);
        let name = PARTS[a].trim_end_matches("_Left");
        // Sides that disagree (a rolled sleeve, a holster): the thinner's, a part a little thin being better than one
        // fatter than its mesh; sides twice apart say nothing but that one of them is wrong: the group's.
        let f = match (own[a], own[b]) {
            (Some(x), Some(y)) if (x - y).abs() <= AGREE.0 * x.max(y) => Some((x + y) * 0.5),
            (Some(x), Some(y)) if x.min(y) * 2.0 >= x.max(y) => {
                notes.push(format!("{name}: the sides' meshes disagree (×{x:.2}, ×{y:.2}), the thinner's"));
                Some(x.min(y))
            }
            (Some(x), Some(y)) => {
                notes.push(format!("{name}: the sides' meshes far apart (×{x:.2}, ×{y:.2}), the group's"));
                None
            }
            (x, y) => x.or(y),
        };
        if let Some(f) = f {
            let f = g * (f / g).clamp(OWN.0, OWN.1);
            (thick[a], thick[b]) = (f, f);
        }
        let t = thickness(&shape(a)) * g;
        match (sections[a].filter(|s| s.round), sections[b].filter(|s| s.round)) {
            (Some(x), Some(y)) if x.centre.distance(flip(y.centre)) <= AGREE.1 * t * 2.0 => {
                let c = (x.centre + flip(y.centre)) * 0.5;
                let c = c.clamp_length_max(OFFSET * t);
                (centre[a], centre[b]) = (Some(c), Some(flip(c)));
            }
            (Some(_), Some(_)) => notes.push(format!("{name}: the sides' meshes lie apart, kept on the bones")),
            _ => notes.push(format!("{name}: the mesh isn't round its bones, kept on them")),
        }
    }
    for t in &mut thick {
        *t = t.max(FLOOR);
    }
    Measured { factors, thick, centre, notes }
}

/// The parts' shapes on the model from the plain fit (`plain`), the factors used and what was left as it was.
pub fn targets(skel: &Skeleton, mesh: &Mesh, phy: Option<&[u8]>, map: &[Option<String>; 21], plain: &Fit, parts: &[usize]) -> (Vec<Target>, Factors, Vec<String>) {
    let j = &plain.shown_joints;
    let mut clouds = clouds(skel, mesh, map, j);
    // The ragdoll's hulls where the model has them (the mesh's details left out by its maker), the mesh elsewhere.
    // A hull only where it lies on its part's mesh: many models borrow another's `.phy` (a citizen's on a fast
    // zombie, hands where another model's are).
    let hulls = phy.and_then(|p| hull_clouds(skel, p, map, j));
    let (mut from_hulls, mut off) = (0, vec![]);
    if let Some(h) = hulls {
        for (k, (c, h)) in clouds.iter_mut().zip(h).enumerate() {
            if h.all.len() < FEWEST {
                continue;
            }
            // A limb's by its cross-section (its spread is its length's), the rest by their spread.
            let fits = match NEXT[k].filter(|_| LIMBS.contains(&k)) {
                Some(n) => {
                    let reach = thickness(&plain.body.parts[k].shape) * REACH;
                    match (section(&h, j[k], j[n], reach).filter(|s| s.round), section(c, j[k], j[n], reach).filter(|s| s.round)) {
                        (Some(hs), Some(ms)) => {
                            (HULL_FITS.0..=HULL_FITS.1).contains(&(hs.radius / ms.radius.max(1e-3))) && hs.centre.distance(ms.centre) <= ms.radius * 0.5
                        }
                        (Some(_), None) => true,
                        _ => false,
                    }
                }
                None => hull_fits(&h.all, &c.all),
            };
            if fits {
                *c = h;
                from_hulls += 1;
            } else {
                off.push(PARTS[k]);
            }
        }
    }
    let Measured { factors, thick, centre: across, mut notes } = measure(&clouds, plain);
    if !off.is_empty() {
        notes.insert(0, format!("the .phy's hulls of {} don't lie on the mesh: measured on the mesh", off.join(", ")));
    }
    notes.insert(0, if from_hulls > 0 { format!("{from_hulls} part(s) measured on the .phy's hulls, the rest on the mesh") } else { "measured on the mesh (no .phy hulls on it)".into() });
    // The feet and hands, both sides each other's mirror images where they nearly are: one way, one size.
    let leaf_of = |k: usize, way: Option<Vec3>| leaf(skel, mesh, map, &clouds[k], plain, k, &plain.body.parts[k].shape, thick[k], way);
    let mut leaves: [Option<Result<Leaf, String>>; 21] = std::array::from_fn(|k| LEAVES.contains(&k).then(|| leaf_of(k, None)));
    let n = Vec3::new(j[1].x - j[4].x, j[1].y - j[4].y, 0.0).normalize_or_zero();
    let flip = |v: Vec3| v - 2.0 * n * n.dot(v);
    for (a, b) in [(3, 6), (16, 20)] {
        if let (Some(Ok(x)), Some(Ok(y))) = (&leaves[a], &leaves[b])
            && x.mesh
            && y.mesh
            && x.way.dot(flip(y.way)) >= LEAF_AGREE
        {
            let w = (x.way + flip(y.way)).normalize();
            (leaves[a], leaves[b]) = (Some(leaf_of(a, Some(w))), Some(leaf_of(b, Some(flip(w)))));
        }
        if let (Some(Ok(x)), Some(Ok(y))) = (&leaves[a], &leaves[b]) {
            let h = (x.half + y.half) * 0.5;
            for k in [a, b] {
                if let Some(Ok(l)) = &mut leaves[k] {
                    l.half = h;
                }
            }
        }
    }
    let out = parts
        .iter()
        .map(|&k| {
            let shape = plain.body.parts[k].shape;
            let now = plain.shown[k].rotation * shape_rot(&shape);
            let centre0 = plain.shown[k].translation;
            let (long, lengthy) = long_axis(&shape);
            let cur = now * long;
            let factor = thick[k];
            let mut boxed = None;
            let (frame, centre) = if let Some(n) = NEXT[k] {
                // Along its bone: where the plain fit has it along the bone, centred across in its mesh.
                let w = (j[n] - j[k]).normalize_or_zero();
                let lie = lengthy && w != Vec3::ZERO && cur.dot(w).abs() > ALIGN;
                let frame = if lie { (Quat::from_rotation_arc(cur, w * cur.dot(w).signum()) * now).normalize() } else { now };
                let centre = if lie { j[k] + w * (centre0 - j[k]).dot(w) + across[k].unwrap_or(Vec3::ZERO) } else { centre0 };
                (frame, centre)
            } else if let Some(l) = leaves[k].take() {
                match l {
                    Ok(l) => {
                        if let Some(n) = l.note {
                            notes.push(format!("{}: {n}", PARTS[k]));
                        }
                        boxed = Some(l.half);
                        (l.frame, l.centre)
                    }
                    Err(why) => {
                        notes.push(format!("{}: {why}, kept", PARTS[k]));
                        (now, centre0)
                    }
                }
            } else if k == 12 {
                // The head: upright as Homer's stands, in the middle of its mesh (the fit puts it where Homer's is on
                // his neck), not far off.
                let upright = upright_head(plain);
                let reach = match shape {
                    Shape::Capsule { radius, half, .. } => radius + half,
                    Shape::Box { half, .. } => half.max_element(),
                };
                let centre = match head_box(&clouds[12], upright) {
                    Some((mid, _)) => centre0 + (mid - centre0).clamp_length_max(reach * HEAD_MOVE),
                    None => {
                        notes.push("Head: too little mesh, kept".into());
                        centre0
                    }
                };
                let bow = now.angle_between(upright).to_degrees();
                if bow > 3.0 {
                    notes.push(format!("Head: turned upright as Homer's stands ({bow:.0}° off with the neck's lean)"));
                }
                (upright, centre)
            } else {
                (now, centre0)
            };
            let shape = match shape {
                Shape::Capsule { rot, radius, half } => Shape::Capsule { rot, radius: radius * factor, half },
                Shape::Box { rot, half } => Shape::Box {
                    rot,
                    half: boxed.unwrap_or_else(|| Vec3::from_array([0, 1, 2].map(|i| if Vec3::AXES[i] == long { half[i] } else { half[i] * factor }))),
                },
            };
            Target { k, centre, frame, shape }
        })
        .collect();
    (out, factors, notes)
}

/// The axis offsets (about their middle) spread along most (the covariance's main eigenvector) and how long they are
/// along it (its variance to the rest's); `None` when they don't spread.
pub fn principal(offs: &[Vec3]) -> Option<(Vec3, f32)> {
    let mut c = Mat3::ZERO;
    for o in offs {
        c += Mat3::from_cols(*o * o.x, *o * o.y, *o * o.z);
    }
    // Power iteration from the axis the offsets spread most along.
    let diag = Vec3::new(c.x_axis.x, c.y_axis.y, c.z_axis.z);
    let mut v = Vec3::AXES[(0..3).max_by(|&a, &b| diag[a].total_cmp(&diag[b]))?];
    for _ in 0..32 {
        v = (c * v).try_normalize()?;
    }
    let main = v.dot(c * v);
    Some((v, main / (diag.x + diag.y + diag.z - main).max(1e-9)))
}

/// A foot's or hand's placement: its frame, centre and box.
struct Leaf {
    frame: Quat,
    centre: Vec3,
    half: Vec3,
    note: Option<String>,
    /// The way it lies along, and whether it was laid over its mesh.
    way: Vec3,
    mesh: bool,
}

/// A foot or a hand laid along its own bones (checked against its mesh; a foot level) and over its mesh: its length
/// and its extents across the mesh's within bounds of Homer's, centred along and across in it, a foot's sole on the
/// mesh's. Without mesh, a hand as thick as its group (`factor`) from its joint along its bones. `given`: the way to lie
/// along (the two sides' made one).
#[allow(clippy::too_many_arguments)]
fn leaf(skel: &Skeleton, mesh: &Mesh, map: &[Option<String>; 21], cloud: &Cloud, plain: &Fit, k: usize, shape: &Shape, factor: f32, given: Option<Vec3>) -> Result<Leaf, String> {
    let Shape::Box { half, .. } = *shape else { return Err("not a box".into()) };
    let foot = FEET.contains(&k);
    let level = |v: Vec3| if foot { v.with_z(0.0) } else { v };
    let joint = plain.shown_joints[k];
    let now = plain.shown[k].rotation * shape_rot(shape);
    let li = (0..3).max_by(|&a, &b| half[a].total_cmp(&half[b])).unwrap_or(1);
    let cur = now * Vec3::AXES[li];
    let turned = |way: Vec3| (Quat::from_rotation_arc(cur, way * cur.dot(way).signum()) * now).normalize();
    let bone_way = leaf_way(skel, mesh, map, k).map(|w| level(w).normalize_or_zero()).filter(|w| *w != Vec3::ZERO);
    let pts = cloud.pick(|_| true);
    if pts.len() < FEWEST {
        // No mesh (a skeleton's hands, a peg leg): a hand from its wrist along its bones (the plain fit's may be far
        // off it); a foot stays as Homer's stands.
        return match bone_way.filter(|_| !foot) {
            Some(w) => {
                let out = Vec3::from_array([0, 1, 2].map(|i| if i == li { half[i] } else { half[i] * factor.clamp(ENDS.0, ENDS.1) }));
                Ok(Leaf { frame: turned(w), centre: joint + w * half[li], half: out, note: Some("no mesh: laid from its joint along its bones".into()), way: w, mesh: false })
            }
            None => Err("too little mesh".into()),
        };
    }
    let keep = inliers(&pts.iter().map(|p| p.distance(joint)).collect::<Vec<_>>());
    let pts: Vec<Vec3> = pts.iter().zip(keep).filter(|x| x.1).map(|x| *x.0).collect();
    // The mesh's way: its long axis (a foot's on the floor, of the vertices below its ankle: a boot's top is a ring
    // round it), pointing off the joint toward its middle; only when the mesh is long (a fist, a boot's ring isn't).
    let mid = median3(&pts);
    let toward = bone_way.unwrap_or(level(mid - joint));
    let sole: Vec<Vec3> = if foot { pts.iter().copied().filter(|p| p.z < joint.z).collect() } else { pts.clone() };
    let sole = if sole.len() >= FEWEST { sole } else { pts.clone() };
    let sole_mid = median3(&sole);
    let mesh_way = principal(&sole.iter().map(|p| level(*p - sole_mid)).collect::<Vec<_>>())
        .filter(|a| a.1 >= LONG_MESH)
        .map_or(Vec3::ZERO, |(a, _)| level(a).normalize_or_zero() * toward.dot(a).signum());
    // A hand's fingers are its way (its mesh's axis may be its palm's width in a fist); a foot's toes when its mesh
    // agrees.
    let (way, note) = match (given, bone_way) {
        (Some(g), _) => (g, None),
        (_, Some(b)) if !foot || mesh_way == Vec3::ZERO || b.dot(mesh_way) >= LEAF_AGREE => (b, None),
        (_, Some(_)) if mesh_way != Vec3::ZERO => (mesh_way, Some("its bones point off its mesh: along the mesh".to_string())),
        _ if mesh_way != Vec3::ZERO => (mesh_way, None),
        _ => return Err("no way to lie along".into()),
    };
    let frame = turned(way);
    let axes = [0, 1, 2].map(|i| frame * Vec3::AXES[i]);
    // Along: the mesh's span from its joint; across: the middle of its span, a foot's sole on the mesh's.
    let span = |i: usize, lo: f32, hi: f32| {
        let mut v: Vec<f32> = pts.iter().map(|p| (*p - joint).dot(axes[i])).collect();
        (quantile(&mut v, lo), quantile(&mut v, hi))
    };
    let mut centre = joint;
    let mut out = half;
    for i in 0..3 {
        let (lo, hi) = span(i, 0.03, 0.97);
        if i == li {
            out[i] = half[i] * ((hi - lo) * 0.5 / half[i].max(1e-3)).clamp(LENGTH.0, LENGTH.1);
            centre += axes[i] * (lo + hi) * 0.5;
            continue;
        }
        // Across, its core's extent (a thumb out, a buckle, a heel's spur left out).
        let core = span(i, 0.1, 0.9);
        out[i] = half[i] * ((core.1 - core.0) * 0.5 / half[i].max(1e-3)).clamp(ENDS.0, ENDS.1);
        centre += axes[i]
            * if foot && axes[i].z.abs() > 0.7 {
                // The sole: the mesh's lowest, the box's bottom on it.
                if axes[i].z > 0.0 { lo + out[i] } else { hi - out[i] }
            } else {
                (lo + hi) * 0.5
            };
    }
    Ok(Leaf { frame, centre, half: out, note, way, mesh: true })
}

/// Where NM's head looks over the model (a unit way, the model's space): Homer's forward as he stands (his hips' line
/// across the up) in the head's frame, the head as shown.
pub fn face(f: &Fit) -> Vec3 {
    let forward = (f.joints[1] - f.joints[4]).with_z(0.0).normalize_or_zero().cross(Vec3::Z);
    f.shown[12].rotation * (f.frames[12].rotation.inverse() * forward)
}

/// Part `k`'s placement on its bone (the edits' offset and turn) that puts its shape at `at` in the frame `want` (the
/// shape's own axes), from the fit as it is with the part's edits `pe`.
pub fn placement(f: &Fit, k: usize, pe: rigs::PartEdit, at: Vec3, want: Quat) -> (Vec3, Vec3) {
    // The part's frame without its placement.
    let t_rot = f.shown[k].rotation * scene::from_angles(pe.turn).inverse();
    let t_pos = f.shown[k].translation - t_rot * pe.offset;
    let turn = scene::angles((t_rot.inverse() * want * shape_rot(&f.body.parts[k].shape).inverse()).normalize());
    // Rounded to a hundredth of a degree: the file's text stays short.
    (t_rot.inverse() * (at - t_pos), (turn * 100.0).round() / 100.0)
}

/// A rig snapped and what to say of it.
pub struct Snapped {
    pub rig: Rig,
    pub status: String,
    pub notes: Vec<String>,
}

/// `rig` with `parts` laid on the model by [`targets`]: their sizes, then their placement on their bones (the other
/// parts' edits and the joints' kept).
#[allow(clippy::too_many_arguments)]
pub fn snap_rig(homer: &Homer, skel: &Skeleton, mesh: &Mesh, phy: Option<&[u8]>, path: &str, rig: &Rig, map: &[Option<String>; 21], parts: &[usize]) -> Result<Snapped, String> {
    let fit = |r: &Rig| {
        let (names, edits) = r.args();
        dms_fit::fit::fit(&homer.build, &homer.body, skel, path, names, edits)
    };
    // The plain fit: the model's joints and masses, Homer's parts as they fit them (no sizes, no placement).
    let mut plain = rig.clone();
    for pe in &mut plain.edits.parts {
        *pe = rigs::PartEdit { mass: pe.mass, ..rigs::PartEdit::default() };
    }
    let f = fit(&plain)?;
    let (ts, g, mut notes) = targets(skel, mesh, phy, map, &f, parts);
    // Only the notes on the parts snapped (and the body's).
    notes.retain(|n| parts.len() == 21 || parts.iter().any(|&k| n.starts_with(PARTS[k]) || n.starts_with(PARTS[k].trim_end_matches("_Left").trim_end_matches("_Right"))));
    let mut out = rig.clone();
    for t in &ts {
        let pe = &mut out.edits.parts[t.k];
        let round = |v: f32| (v * 1e4).round() / 1e4;
        match t.shape {
            Shape::Capsule { radius, half, .. } => {
                pe.radius = Some(round(radius));
                pe.half = Some(round(half));
            }
            Shape::Box { half, .. } => pe.boxed = Some(half.to_array().map(round).into()),
        }
    }
    let f = fit(&out)?;
    for t in &ts {
        let (offset, turn) = placement(&f, t.k, out.edits.parts[t.k], t.centre, t.frame);
        out.edits.parts[t.k].offset = offset;
        out.edits.parts[t.k].turn = turn;
    }
    let mut status = format!(
        "{} part(s) laid on the bones; thickness to Homer's: body ×{:.2}, legs ×{:.2}, arms ×{:.2}, torso ×{:.2}, pelvis ×{:.2}, neck ×{:.2}, head ×{:.2}",
        ts.len(),
        g.body,
        g.legs,
        g.arms,
        g.torso,
        g.pelvis,
        g.neck,
        g.head
    );
    if let Some(n) = notes.first() {
        status += &format!("; {n}");
        if notes.len() > 1 {
            status += &format!(" (+{} more)", notes.len() - 1);
        }
    }
    Ok(Snapped { rig: out, status, notes })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ring of `n` points round `c` (radius `r`) across the Z axis, from angle `a0` over `span` radians.
    fn ring_pts(c: Vec2, r: f32, n: usize, a0: f32, span: f32) -> Vec<Vec3> {
        (0..n)
            .map(|i| {
                let a = a0 + span * i as f32 / n as f32;
                Vec3::new(c.x + r * a.cos(), c.y + r * a.sin(), 0.0)
            })
            .collect()
    }

    #[test]
    fn medians() {
        assert_eq!(median(vec![1.0, 3.0]), Some(2.0));
        assert_eq!(median(vec![5.0, 1.0, 3.0]), Some(3.0));
        assert_eq!(median(vec![]), None);
    }

    #[test]
    fn a_ring_with_a_pouch() {
        // A limb round an off-centre bone, a pouch's vertices on one side: the circle is the limb's.
        let mut pts = ring_pts(Vec2::new(0.02, -0.01), 0.06, 48, 0.0, std::f32::consts::TAU);
        pts.extend((0..10).map(|i| Vec3::new(0.15 + i as f32 * 0.003, 0.0, 0.0)));
        let s = ring(&pts, Vec3::Z).unwrap();
        assert!(s.centre.distance(Vec3::new(0.02, -0.01, 0.0)) < 0.005, "{:?}", s.centre);
        assert!((s.radius - 0.06).abs() < 0.005, "{}", s.radius);
        assert!(s.round);
    }

    #[test]
    fn a_ring_with_a_holster() {
        // A thigh round its bone and a holster as dense as it beside it: the thigh, not the holster.
        let mut pts = ring_pts(Vec2::ZERO, 0.08, 40, 0.0, std::f32::consts::TAU);
        pts.extend(ring_pts(Vec2::new(0.16, 0.02), 0.03, 50, 0.0, std::f32::consts::TAU));
        let s = ring(&pts, Vec3::Z).unwrap();
        assert!(s.centre.length() < 0.01 && (s.radius - 0.08).abs() < 0.01 && s.round, "{:?} {}", s.centre, s.radius);
    }

    #[test]
    fn an_arc_is_not_round() {
        // A mesh on one side of its bone (a sleeve's edge): no centre to trust.
        let s = ring(&ring_pts(Vec2::ZERO, 0.05, 30, 0.0, 1.5), Vec3::Z);
        assert!(s.is_none_or(|s| !s.round));
    }

    #[test]
    fn a_foot_is_long_a_ring_is_not() {
        let foot: Vec<Vec3> = (0..40).map(|i| Vec3::new((i % 10) as f32 * 0.025 - 0.11, (i / 10) as f32 * 0.02 - 0.03, 0.0)).collect();
        let (axis, long) = principal(&foot).unwrap();
        assert!(axis.x.abs() > 0.99 && long > LONG_MESH, "{axis:?} {long}");
        let (_, round) = principal(&ring_pts(Vec2::ZERO, 0.05, 32, 0.0, std::f32::consts::TAU)).unwrap();
        assert!(round < LONG_MESH, "{round}");
    }

    #[test]
    fn outliers_left_out() {
        let keep = inliers(&[1.0, 1.1, 0.9, 1.05, 0.95, 5.0]);
        assert_eq!(keep, vec![true, true, true, true, true, false]);
    }
}
