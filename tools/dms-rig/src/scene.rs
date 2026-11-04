//! What the 3D view shows: NM's parts as the fork hulls them (`dms_vphysics::hull`), picking them by a ray, Source's
//! angles, and the model's mesh posed from NM's parts (the bones as the game poses them: the parts' own from their
//! parts, the rest after their parents, docs/adr/0030).

use delirium::prelude::*;
use dms_fit::M;
use dms_fit::fit::Fit;
use dms_fit::skeleton::Skeleton;
use dms_game::body::Shape;

use crate::mesh::Mesh;
use crate::render::{Batch, Vertex};

/// A part's colour: the legs, the spine, the arms told apart, sides by shade.
pub fn part_color(k: usize, alpha: f32) -> [f32; 4] {
    let c = match k {
        0 => [0.85, 0.75, 0.3],
        1..=3 => [0.3, 0.6, 0.95],
        4..=6 => [0.2, 0.45, 0.8],
        7..=10 => [0.9, 0.55, 0.25],
        11 | 12 => [0.95, 0.85, 0.5],
        13..=16 => [0.4, 0.85, 0.45],
        _ => [0.25, 0.7, 0.35],
    };
    [c[0], c[1], c[2], alpha]
}

/// The hull of a shape (points in the part's frame, triangles), cached by the shape.
#[derive(Default)]
pub struct Hulls {
    cache: Vec<(Shape, Vec<Vec3>, Vec<[usize; 3]>)>,
}

impl Hulls {
    pub fn get(&mut self, s: &Shape) -> (&[Vec3], &[[usize; 3]]) {
        let i = match self.cache.iter().position(|c| c.0 == *s) {
            Some(i) => i,
            None => {
                let p = dms_vphysics::hull::points(s);
                let t = dms_vphysics::hull::hull(&p);
                if self.cache.len() > 256 {
                    self.cache.clear();
                }
                self.cache.push((*s, p, t));
                self.cache.len() - 1
            }
        };
        let c = &self.cache[i];
        (&c.1, &c.2)
    }
}

/// Where a ray (origin, direction) first meets a part's shape placed at `f`: the distance along it.
pub fn ray_shape(o: Vec3, d: Vec3, f: Transform, s: &Shape) -> Option<f32> {
    let inv = f.compute_affine().inverse();
    let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
    match *s {
        Shape::Capsule { rot, radius, half } => {
            let (o, d) = (rot.inverse() * lo, rot.inverse() * ld);
            // Sampled along the ray: the nearest approach to the segment within the radius.
            let mut best: Option<f32> = None;
            let n = 200;
            for i in 0..n {
                let t = i as f32 * 0.025;
                let p = o + d * t;
                let q = Vec3::new(0.0, p.y.clamp(-half, half), 0.0);
                if p.distance(q) <= radius {
                    best = Some(t);
                    break;
                }
            }
            best
        }
        Shape::Box { rot, half } => {
            let (o, d) = (rot.inverse() * lo, rot.inverse() * ld);
            let (mut t0, mut t1) = (0.0f32, f32::MAX);
            for i in 0..3 {
                if d[i].abs() < 1e-8 {
                    if o[i].abs() > half[i] {
                        return None;
                    }
                    continue;
                }
                let (a, b) = ((-half[i] - o[i]) / d[i], (half[i] - o[i]) / d[i]);
                t0 = t0.max(a.min(b));
                t1 = t1.min(a.max(b));
            }
            (t0 <= t1).then_some(t0)
        }
    }
}

/// Source's angles (pitch, yaw, roll; degrees) of a rotation (`MatrixAngles`, as the module's `ragdolls::angles`).
pub fn angles(q: Quat) -> Vec3 {
    let m = Mat3::from_quat(q);
    let (fwd, left, up) = (m.x_axis, m.y_axis, m.z_axis);
    let xy = (fwd.x * fwd.x + fwd.y * fwd.y).sqrt();
    let (pitch, yaw, roll) = if xy > 0.001 {
        ((-fwd.z).atan2(xy), fwd.y.atan2(fwd.x), left.z.atan2(up.z))
    } else {
        ((-fwd.z).atan2(xy), (-left.x).atan2(left.y), 0.0)
    };
    Vec3::new(pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees())
}

/// The rotation of Source's angles (`AngleMatrix`; as `rigs::PartEdit::adjust`).
pub fn from_angles(a: Vec3) -> Quat {
    let [p, y, r] = a.to_array().map(f32::to_radians);
    (Quat::from_rotation_z(y) * Quat::from_rotation_y(p) * Quat::from_rotation_x(r)).normalize()
}

/// The mesh as a batch in metres, its own pose.
pub fn mesh_batch(m: &Mesh, color: [f32; 4]) -> Batch {
    let verts = m.pos.iter().zip(&m.normal).map(|(p, n)| Vertex { pos: (Vec3::from_array(*p) * M).to_array(), normal: *n, color }).collect();
    Batch { verts, idx: m.tris.iter().flatten().copied().collect(), lines: false, on_top: false }
}

/// Every bone's frame (m, world) with the body's parts at `parts` (their frames): the parts' own bones and the
/// carried physics bones from their parts (`Fit::posed`), the rest after their parents as in the mesh's bind (the
/// roots after the pelvis' part).
pub fn pose_bones(s: &Skeleton, fit: &Fit, parts: &[(Vec3, Quat)]) -> Vec<(Vec3, Quat)> {
    let bind = |b: usize| (s.bones[b].pose.0 * M, s.bones[b].pose.1);
    let mut out: Vec<(Vec3, Quat)> = Vec::with_capacity(s.bones.len());
    let pelvis = fit.shown[0];
    for b in 0..s.bones.len() {
        let posed = fit.posed.iter().find(|p| p.bone.eq_ignore_ascii_case(&s.bones[b].name));
        let f = match (posed, s.bones[b].parent) {
            (Some(p), _) => {
                let (q, r) = parts[p.part];
                (q + r * p.at, (r * p.turn).normalize())
            }
            (None, Some(par)) => {
                let ((pp, pq), (bp, bq)) = (bind(par), bind(b));
                let (wp, wq) = out[par];
                (wp + wq * (pq.inverse() * (bp - pp)), (wq * pq.inverse() * bq).normalize())
            }
            (None, None) => {
                let (bp, bq) = bind(b);
                let (wp, wq) = parts[0];
                let inv = pelvis.rotation.inverse();
                (wp + wq * (inv * (bp - pelvis.translation)), (wq * inv * bq).normalize())
            }
        };
        out.push(f);
    }
    out
}

/// The mesh skinned to bones posed at `world` (from its bind `poseToBone`), as a batch.
pub fn skinned_batch(m: &Mesh, s: &Skeleton, world: &[(Vec3, Quat)], color: [f32; 4]) -> Batch {
    let skin: Vec<Mat4> = s
        .bones
        .iter()
        .zip(world)
        .map(|(b, &(p, q))| Mat4::from_rotation_translation(q, p) * Mat4::from_rotation_translation(b.pose.1, b.pose.0 * M).inverse())
        .collect();
    let verts = m
        .pos
        .iter()
        .zip(&m.normal)
        .zip(&m.bones)
        .map(|((p, n), w)| {
            let (p, n) = (Vec3::from_array(*p) * M, Vec3::from_array(*n));
            let (mut pos, mut nor, mut sum) = (Vec3::ZERO, Vec3::ZERO, 0.0);
            for &(b, wt) in w.iter().filter(|x| x.1 > 0.0) {
                if let Some(k) = skin.get(b as usize) {
                    pos += k.transform_point3(p) * wt;
                    nor += k.transform_vector3(n) * wt;
                    sum += wt;
                }
            }
            if sum <= 0.0 {
                (pos, nor) = (p, n);
            } else {
                pos /= sum;
            }
            Vertex { pos: pos.to_array(), normal: nor.normalize_or(Vec3::Z).to_array(), color }
        })
        .collect();
    Batch { verts, idx: m.tris.iter().flatten().copied().collect(), lines: false, on_top: false }
}
