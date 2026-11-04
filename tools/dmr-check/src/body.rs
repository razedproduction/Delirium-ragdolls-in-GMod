//! `dmr-check body`: a body fitted to a GMod model (ROADMAP M11), which the fork makes natively (no `.phy`): its
//! build against the SDK's own fit, then its native body on the bench against the build, then dropped without NM.

use std::time::Duration;

use delirium::prelude::*;
use delirium_physics as sdk;
use dms_game::body::Body;

/// Build tolerance (ROADMAP M3).
const BUILD_TOL: f32 = 1e-5;
/// In the game: positions (m), rotations (rad), inertia (relative, Frobenius).
const POS_TOL: f32 = 0.001;
const ROT_TOL: f32 = 0.5 * std::f32::consts::PI / 180.0;
const INERTIA_TOL: f32 = 0.01;

/// Worst gaps, by what was compared.
#[derive(Default)]
struct Worst(Vec<(String, f32, String)>);

impl Worst {
    fn add(&mut self, what: &str, gap: f32, at: &str) {
        match self.0.iter_mut().find(|w| w.0 == what) {
            Some(w) if gap > w.1 || gap.is_nan() => (w.1, w.2) = (gap, at.into()),
            Some(_) => {}
            None => self.0.push((what.into(), gap, at.into())),
        }
    }

    fn report(&self, tol: f32) -> bool {
        let mut ok = true;
        for (what, gap, at) in &self.0 {
            let pass = *gap <= tol;
            ok &= pass;
            println!("  {:<28} {:>12.3e}  {:<18} {}", what, gap, at, if pass { "ok" } else { "OFF" });
        }
        ok
    }
}

fn qgap(a: Quat, b: Quat) -> f32 {
    // atan2 of the relative turn's parts: acos near 1 is all rounding in f32.
    let d = (a.inverse() * b).normalize();
    2.0 * Vec3::new(d.x, d.y, d.z).length().atan2(d.w.abs())
}

fn vgap(a: Vec3, b: Vec3) -> f32 {
    (a - b).length()
}

/// The body against the SDK's C API on `dump` (part info and frames, an agent's joints at the origin); with `fit`, on
/// the SDK's own fit of that build to those joints (`nm_build_fit`).
fn against_sdk(body: &Body, dump: &str, fit: Option<&[f32]>, w: &mut Worst) -> Result<(), String> {
    let c = std::ffi::CString::new(dump).map_err(|e| e.to_string())?;
    // SAFETY: the SDK's own calls on a build and an agent made here, with outputs as big as they write.
    unsafe {
        let mut b = sdk::nm_build_from_text(c.as_ptr());
        if b.is_null() {
            return Err("nm_build_from_text failed".into());
        }
        if let Some(j) = fit {
            let fitted = sdk::nm_build_fit(b, j.as_ptr());
            sdk::nm_build_destroy(b);
            if fitted.is_null() {
                return Err("nm_build_fit failed".into());
            }
            b = fitted;
        }
        let a = sdk::nm_agent_create(b, [0.0f32; 3].as_ptr(), 0.0, 0.3);
        let placed = body.placed(Vec3::ZERO, 0.0);
        let mut r = Ok(());
        for (i, p) in body.parts.iter().enumerate() {
            let mut name = [0u8; 64];
            sdk::nm_part_name(i as i32, name.as_mut_ptr().cast(), 64);
            let name = std::ffi::CStr::from_bytes_until_nul(&name).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let (mut m, mut rad, mut half, mut parent) = (0.0, 0.0, 0.0, -1);
            let (mut pos, mut rot) = ([0.0f32; 3], [0.0f32; 4]);
            sdk::nm_build_part_info(b, i as i32, &mut m, &mut rad, &mut half, &mut parent);
            sdk::nm_build_part_frame(b, i as i32, pos.as_mut_ptr(), rot.as_mut_ptr());
            if name != p.name || parent != p.parent.map_or(-1, |q| q as i32) {
                r = Err(format!("part {i}: the SDK has {name} (parent {parent}), the body {} ({:?})", p.name, p.parent));
                break;
            }
            w.add("sdk: part mass (kg)", (m - p.mass).abs(), p.name);
            w.add("sdk: part radius, half (m)", (rad - p.radius).abs().max((half - p.half).abs()), p.name);
            w.add("sdk: part frame position (m)", vgap(Vec3::from_array(pos), p.pos), p.name);
            w.add("sdk: part frame rotation (rad)", qgap(Quat::from_array(rot), p.rot), p.name);
        }
        let poses: Vec<(Vec3, Quat)> = placed.clone();
        for j in &body.joints {
            let (mut parent, mut pivot, mut frame, mut limits, mut arm) = (-1, [0.0f32; 3], [0.0f32; 4], [0.0f32; 3], 0.0);
            let kind = sdk::nm_agent_get_joint(a, j.part as i32, &mut parent, pivot.as_mut_ptr(), frame.as_mut_ptr(), limits.as_mut_ptr(), &mut arm);
            let name = body.parts[j.part].name;
            if kind != if j.hinge { 2 } else { 1 } || parent != j.parent as i32 {
                r = Err(format!("joint {name}: the SDK has kind {kind}, parent {parent}"));
                break;
            }
            let (piv, frames) = body.joint_world(j, &poses);
            w.add("sdk: joint pivot (m)", vgap(Vec3::from_array(pivot), piv), name);
            w.add("sdk: joint frame (rad)", qgap(Quat::from_array(frame), frames[0]), name);
            let n = if j.hinge { 2 } else { 3 };
            w.add("sdk: joint limits (rad)", (0..n).map(|i| (limits[i] - j.limits[i]).abs()).fold(0.0, f32::max), name);
        }
        sdk::nm_agent_destroy(a);
        sdk::nm_build_destroy(b);
        r
    }
}

/// One part as the game reports it (`DMR.RagdollReport`).
struct GamePart {
    name: String,
    pos: Vec3,
    rot: Quat,
    origin: Vec3,
    mass: f32,
    inertia: Vec3,
    damping: [f32; 2],
    /// The tensor Box3D turns the body with (`DMR.BodyInertia`), if the fork gives it.
    tensor: Option<Mat3>,
}

fn report(text: &str) -> Result<((Vec3, f32), Vec<GamePart>), String> {
    let mut spawn = None;
    let mut parts = vec![];
    for line in text.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        let f = |i: usize| w.get(i).and_then(|x| x.parse::<f32>().ok()).ok_or(format!("bad report line: {line}"));
        match w.first().copied() {
            Some("spawn") => spawn = Some((Vec3::new(f(1)?, f(2)?, f(3)?), f(4)?)),
            Some("part") => parts.push(GamePart {
                name: w.get(1).copied().unwrap_or("").into(),
                pos: Vec3::new(f(2)?, f(3)?, f(4)?),
                rot: Quat::from_xyzw(f(5)?, f(6)?, f(7)?, f(8)?).normalize(),
                origin: Vec3::new(f(9)?, f(10)?, f(11)?),
                mass: f(12)?,
                inertia: Vec3::new(f(13)?, f(14)?, f(15)?),
                damping: [f(16)?, f(17)?],
                tensor: (w.len() >= 27).then(|| (18..27).map(f).collect::<Result<Vec<f32>, _>>()).transpose()?
                    .map(|v| Mat3::from_cols_array(&v.try_into().unwrap_or([0.0; 9])).transpose()),
            }),
            Some("error") => return Err(line.into()),
            _ => {}
        }
    }
    Ok((spawn.ok_or(format!("no spawn line in: {text}"))?, parts))
}

/// A body's report from the game (`spawn`, `part` and `joints` lines) against NM's body standing where it was spawned:
/// frames, masses, damping, inertia, joints. `bones`: its objects are a model's, each bone's origin on its joint's
/// pivot (a model's own ragdoll; a native body's have no bone).
fn against_game(body: &Body, text: &str, bones: bool) -> Result<bool, String> {
    let ((at, yaw), parts) = report(text)?;
    let (joints, objects) = text.lines().find_map(|l| l.strip_prefix("joints ")).and_then(|l| {
        let mut w = l.split_whitespace().filter_map(|x| x.parse::<usize>().ok());
        Some((w.next()?, w.next()?))
    }).unwrap_or((0, 0));
    let want = body.placed(at, yaw.to_radians());
    let (mut wpos, mut wrot, mut wmass) = (Worst::default(), Worst::default(), Worst::default());
    let mut inertia = Worst::default();
    let mut diag = Worst::default();
    let ok = parts.len() == body.parts.len() && objects == body.parts.len() && joints == body.joints.len();
    for ((p, g), (pos, rot)) in body.parts.iter().zip(&parts).zip(&want) {
        if g.name != p.name {
            return Err(format!("the game's part {} where NM's {} is", g.name, p.name));
        }
        wpos.add("frame position (m)", vgap(g.pos, *pos), p.name);
        // The bone (the PhysObj's origin) sits on the joint's pivot, the centre of mass on the part's centre.
        if bones {
            let k = body.parts.iter().position(|q| q.name == p.name).unwrap_or(0);
            wpos.add("bone origin off NM's pivot (m)", vgap(g.origin, *pos + *rot * body.bone_offset(k)), p.name);
        }
        wrot.add("frame rotation (rad)", qgap(g.rot, *rot), p.name);
        wmass.add("mass (relative)", ((g.mass - p.mass) / p.mass).abs(), p.name);
        wmass.add("damping", g.damping[0].abs().max(g.damping[1].abs()), p.name);
        // Box3D's tensor: the fork's own (VBOX3D_NM_003), else GetInertia's diagonal.
        let ours = g.tensor.unwrap_or(Mat3::from_diagonal(g.inertia));
        let frob = |m: Mat3| m.to_cols_array().iter().map(|x| x * x).sum::<f32>().sqrt();
        inertia.add("inertia tensor (relative)", frob(ours - p.inertia) / frob(p.inertia), p.name);
        let d = Vec3::new(p.inertia.x_axis.x, p.inertia.y_axis.y, p.inertia.z_axis.z);
        diag.add("inertia diagonal (relative)", ((g.inertia - d) / d).abs().max_element(), p.name);
    }
    println!("  {} physics objects, {} of {} joints found by FindConstraint, every joint bound (dms_vphysics::body)", objects, joints, body.joints.len());
    let pos_ok = wpos.report(POS_TOL) & wrot.report(ROT_TOL) & wmass.report(1e-6);
    let diag_ok = diag.report(INERTIA_TOL);
    let tensor_ok = inertia.report(INERTIA_TOL);
    if !tensor_ok {
        println!("  NM's inertia tensors have off-diagonal terms in the parts' frames: Box3D needs them whole (VBOX3D_NM_003's");
        println!("  SetBodyInertia); an older fork keeps only the diagonal");
        for p in &body.parts {
            let t = p.inertia;
            println!("    {:16} NM [{:.5} {:.5} {:.5} | {:.5} {:.5} {:.5}] (diagonal | xy xz yz)", p.name, t.x_axis.x, t.y_axis.y,
                     t.z_axis.z, t.y_axis.x, t.z_axis.x, t.z_axis.y);
        }
    }
    Ok(ok && pos_ok && diag_ok && tensor_ok)
}

/// ROADMAP M11: `dmr-check body <model>`, the body check of a body fitted to a GMod model (M10), which the fork
/// makes natively (no `.phy`). The module gives the 21 joints it fitted NM's build to (`DMR.FitJoints`, the model's
/// skeleton in the build's stance: the mapping's input); here NM's build (`homer`, Homer's from the game as `dms-game`
/// reads it: the fit's template) is fitted to them as the module does, and: 1. against the SDK's C API fitting that
/// build's dump (`homer_dump`, `dms-game`'s, which the golden checkout holds to the Playroom's NM_DUMP byte for byte)
/// to the same joints (`nm_build_fit`, 1e-5); 2. on the bench, the native body made standing (`DMR.RagdollBody`,
/// without NM) against it (frames, masses, inertia, joints), then dropped without NM.
/// (A fitted build's own dump can't be the reference: it doesn't carry the parts' sizes.)
pub fn fitted(model: &str, homer: (&delirium::ragdoll::Build, &[Transform]), homer_dump: &str) -> Result<bool, String> {
    let bench = dmr_ctl::bench()?;
    let mut ch = dmr_ctl::Channel::bench(&bench)?;
    let q = format!("{model:?}");
    let text = ch.request("lua", &format!("local f, why = DMR_RagdollFit({q}) if not f then return 'error ' .. tostring(why) end return DMR.FitJoints({q})"),
                          Duration::from_secs(30))?;
    if text.starts_with("error") {
        return Err(text);
    }
    let joints: Vec<f32> = text.split_whitespace().map(|x| x.parse::<f32>().map_err(|_| format!("bad joints: {text}"))).collect::<Result<_, _>>()?;
    if joints.len() != 63 {
        return Err(format!("{} numbers for 21 joints: {text}", joints.len()));
    }
    let at: Vec<Vec3> = joints.chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
    // As the module fits: from Homer's body's frames (data.rs, ragdolls.rs `fit`).
    let frames: Vec<Transform> = Body::new(homer.0, homer.1).parts.iter().map(|p| Transform::from_translation(p.pos).with_rotation(p.rot)).collect();
    let (b, frames) = homer.0.fit(&frames, &at);
    let body = Body::new(&b, &frames);
    println!("dmr-check: body 1/2: NM's build fitted to {model}'s joints against the SDK's nm_build_fit, tolerance {BUILD_TOL:e}");
    let mut w = Worst::default();
    against_sdk(&body, homer_dump, Some(&joints), &mut w)?;
    println!("  {} parts, {} joints ({} hinges), {} self-collision pairs, {:.1} kg", body.parts.len(), body.joints.len(),
             body.joints.iter().filter(|j| j.hinge).count(), body.pairs.len(), body.parts.iter().map(|p| p.mass).sum::<f32>());
    let build_ok = w.report(BUILD_TOL);
    println!("dmr-check: body 2/2: in the game (the bench, dmr-ctl): the native body made standing against the build");
    let spawn = format!(r#"
local model = {q}
local sp = ents.FindByClass("info_player_start")[1]
local at = sp and sp:GetPos() or Vector(0, 0, 0)
local floor = DMR_FloorAt(util.TraceLine({{ start = at + Vector(0, 0, 64), endpos = at - Vector(0, 0, 4096) }}).HitPos + Vector(120, 40, 0))
if IsValid(DMR_GOLDEN_FIT) then DMR_GOLDEN_FIT:Remove() end
local rag = ents.Create("prop_ragdoll")
rag:SetModel(model)
rag:SetPos(floor)
rag:Spawn()
rag:Activate()
rag.DMR_Bench = true
local ok, handle = pcall(DMR.RagdollBody, model, rag:GetPhysicsObjectNum(0), {{ pos = floor, yaw = 37 }})
if not ok then rag:Remove() return "error " .. tostring(handle) end
-- The model's own objects out of the way, as with NM on it (ragdolls.lua).
rag:RemoveInternalConstraint(-1)
for i = 0, rag:GetPhysicsObjectCount() - 1 do
	local p = rag:GetPhysicsObjectNum(i)
	p:EnableMotion(false)
	p:EnableCollisions(false)
	p:EnableGravity(false)
end
rag.DMR_GoldenHandle = handle
rag:CallOnRemove("DMR_GoldenFit", function() DMR.RagdollRemove(handle) end)
DMR_GOLDEN_FIT = rag
return string.format("spawn %.9g %.9g %.9g %.9g\n", floor.x * 0.0254, floor.y * 0.0254, floor.z * 0.0254, 37) .. DMR.RagdollReport(model, handle)
"#);
    let text = ch.request("lua", &spawn, Duration::from_secs(30))?;
    let ok = against_game(&body, &text, false)?;
    let dropped = drop_test(&mut ch, &body, &format!(r#"
local rag = DMR_GOLDEN_FIT
if not IsValid(rag) then return "error the ragdoll's gone" end
local model, handle = {q}, rag.DMR_GoldenHandle
local out = {{ "spawn 0 0 0 0", DMR.RagdollReport(model, handle) }}
-- The fastest bone of the model NM's parts pose (their origins' speed).
local fastest = 0
for _, b in pairs(DMR.PoseModel(model, handle)) do fastest = math.max(fastest, Vector(b.vel.x, b.vel.y, b.vel.z):Length()) end
out[#out + 1] = "speed " .. fastest
-- Posed on the server from the parts (ragdolls.lua): nothing of the model drawn off them to check here.
out[#out + 1] = "drawn 0"
rag:Remove()
return table.concat(out, "\n")
"#))?;
    Ok(build_ok && ok && dropped)
}

/// A body, left without NM to fall from its standing pose for a few seconds: every body finite and slow at the
/// end, every joint's two pivots together, every joint within NM's limits (with a margin). `lua` reports it.
fn drop_test(ch: &mut dmr_ctl::Channel, body: &Body, lua: &str) -> Result<bool, String> {
    const SECONDS: u64 = 4;
    println!("  dropped without NM for {SECONDS} s:");
    std::thread::sleep(Duration::from_secs(SECONDS));
    let text = ch.request("lua", lua, Duration::from_secs(30))?;
    let (_, parts) = report(&text)?;
    let speed: f32 = text.lines().find_map(|l| l.strip_prefix("speed ")).and_then(|s| s.trim().parse().ok()).unwrap_or(f32::NAN);
    let drawn: f32 = text.lines().find_map(|l| l.strip_prefix("drawn ")).and_then(|s| s.trim().parse().ok()).unwrap_or(f32::NAN);
    let poses: Vec<(Vec3, Quat)> = parts.iter().map(|g| (g.pos, g.rot)).collect();
    let finite = poses.iter().all(|(p, q)| p.is_finite() && q.is_finite());
    let mut w = Worst::default();
    let mut over = Worst::default();
    for j in &body.joints {
        let name = body.parts[j.part].name;
        let (pp, pr) = poses[j.parent];
        let (cp, cr) = poses[j.part];
        w.add("joint pivots apart (m)", vgap(pp + pr * j.anchor_parent, cp + cr * j.anchor_child), name);
        let rel = ((pr * j.frame_parent).inverse() * (cr * j.frame_child)).normalize();
        let rel = if rel.w < 0.0 { -rel } else { rel };
        // Twist about the frame's Z and the swing that's left; a hinge only turns about Z.
        let twist = Quat::from_xyzw(0.0, 0.0, rel.z, rel.w).normalize();
        let swing = rel * twist.inverse();
        let t = 2.0 * twist.z.atan2(twist.w);
        let s = swing.to_scaled_axis();
        let beyond = if j.hinge {
            (j.limits[0] - t).max(t - j.limits[1]).max(0.0).max(s.length())
        } else {
            // NM's ellipse (ragdoll.rs lean_limit): limits[0] bounds the swing about the parent frame's Y, [1] about X.
            let e = ((s.y / j.limits[0].max(1e-3)).powi(2) + (s.x / j.limits[1].max(1e-3)).powi(2)).sqrt();
            ((e - 1.0) * j.limits[0].min(j.limits[1])).max(t.abs() - j.limits[2]).max(0.0)
        };
        over.add("joint past NM's limits (rad)", beyond, name);
        if beyond > 0.0 {
            println!("    {name:16} {} twist {:+.1} deg, swing about x {:+.1} y {:+.1} deg; NM's limits (y, x, twist) {:.1} {:.1} {:.1} deg: {:.1} deg past",
                if j.hinge { "hinge" } else { "ball " }, t.to_degrees(), s.x.to_degrees(), s.y.to_degrees(), j.limits[0].to_degrees(),
                j.limits[1].to_degrees(), j.limits[2].to_degrees(), beyond.to_degrees());
        }
    }
    println!("    every body finite: {}; fastest body {speed:.3} m/s; bones drawn off their bodies by {:.4} m at most",
             if finite { "yes" } else { "NO" }, drawn);
    let a = w.report(0.02);
    // Box3D's soft limits and NM's own margins: 5 degrees.
    let b = over.report(5.0f32.to_radians());
    // Source draws a ragdoll's child bone at a fixed offset from its parent's: off the body if the bone isn't on the pivot.
    Ok(finite && speed < 0.5 && drawn < 0.005 && a && b)
}
