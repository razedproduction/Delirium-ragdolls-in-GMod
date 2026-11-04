//! GMod's ragdolls (ROADMAP M9–M12, docs/adr/0025, 0027, 0030): a model's skeleton read from its `.mdl`, mapped onto
//! NM's 21 joints by its rig or a player's mapping (`rigs`) and Homer's build fitted to them (`Build::fit`, as the
//! SDK's Unity bridge does), the fitted
//! body made by the fork natively (`VBOX3D_NM_005`/`_006`) in the model's ragdoll's environment, bound and driven in
//! FOLLOW as Homer is, the model's bones posed from its parts.
//!
//! Fitted bodies are kept by the model's path and its checksum for the session; nothing of a model is written
//! anywhere. The swap (`dmr_attach`): the native body takes the ragdoll's pose and every part's velocity, and gives
//! them back to the model's objects on `dmr_detach` (`DMR.PoseModel`).

use std::collections::HashMap;
use std::ffi::{CStr, c_int};
use std::sync::{Arc, Mutex};

use delirium::prelude::*;
use delirium::ragdoll::{Build, Ragdoll};

use crate::data;
use crate::ffi::vphysics_nm::{self, IPhysicsObject, Version};
use dms_vphysics::body::{NativeBody, PartState};
use crate::follow::BodyDef;
use crate::lua::{self, Api, LuaState, TTABLE, native};
use dms_fit::fit::{FINISH_MAX, Fit, Posed, bind_report, height};
use dms_fit::rigs;
use dms_fit::skeleton::{Skeleton, i32_at, read_mdl};
pub use dms_fit::M;

pub const NATIVES: &[(&CStr, lua::CFunction)] = &[
    (c"ModelSkeleton", model_skeleton),
    (c"RagdollFit", ragdoll_fit),
    (c"ModelClassify", model_classify),
    (c"RagdollForget", ragdoll_forget),
    (c"RagdollShapes", ragdoll_shapes),
    (c"RagdollParts", ragdoll_parts),
    (c"FitDump", fit_dump),
    (c"FitLead", fit_lead),
    (c"FitJoints", fit_joints),
    (c"RagdollBody", ragdoll_body),
    (c"RagdollFollow", ragdoll_follow),
    (c"RagdollWarm", ragdoll_warm),
    (c"RagdollRemove", ragdoll_remove),
    (c"RagdollReport", ragdoll_report),
    (c"PoseModel", pose_model),
];


/// The parts a ragdoll may have no physics object for (ValveBiped has none for these), each between two that it has
/// (or that come before it here) and how far along: where their pose comes from when NM takes over the ragdoll's.
const BETWEEN: [(usize, usize, usize, f32); 6] = [(7, 0, 9, 1.0 / 3.0), (8, 0, 9, 2.0 / 3.0), (10, 9, 12, 1.0 / 3.0), (11, 9, 12, 2.0 / 3.0), (13, 10, 14, 0.5), (17, 10, 18, 0.5)];

/// A body fitted to a model (`dms_fit::fit`), as the agents drive it (`BodyDef`).
pub struct Fitted {
    pub def: Arc<BodyDef>,
    pub fit: Fit,
}

impl std::ops::Deref for Fitted {
    type Target = Fit;
    fn deref(&self) -> &Fit {
        &self.fit
    }
}

/// Homer's build fitted to a model (`dms_fit::fit` on the loaded data's Homer).
fn fit(d: &data::Data, s: &Skeleton, model: &str, names: Option<&[(String, String)]>, edits: Option<&rigs::Edits>) -> Result<Fitted, String> {
    let fit = dms_fit::fit::fit(&d.homer, &d.body, s, model, names, edits)?;
    Ok(Fitted { def: Arc::new(BodyDef { build: fit.build.clone(), body: fit.body.clone(), fitted: true }), fit })
}

static FITS: Mutex<Option<HashMap<String, Arc<Fitted>>>> = Mutex::new(None);

fn fitted(key: &str) -> Result<Arc<Fitted>, String> {
    FITS.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|m| m.get(key).cloned()).ok_or_else(|| format!("{key}: not fitted (DMR.RagdollFit)"))
}

/// Drops the fitted bodies (the module's unload, docs/adr/0021).
pub fn unload() {
    *FITS.lock().unwrap_or_else(|e| e.into_inner()) = None;
    // The map's environment took the native bodies with it.
    *NATIVE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

native!(model_skeleton => model_skeleton_impl);
/// `DMR.ModelSkeleton(mdl)`: the `.mdl`'s bytes (`file.Read(model, "GAME")`) read: `{name, version, checksum,
/// bones = {{name, parent, pos, rot, mesh_pos, mesh_rot, physics, pose_gap, pose_turn}, ...}}`, `parent` 0-based (-1
/// for a root), `pos` and `rot` the bind pose in the model's space (units, quaternion), `mesh_pos` and `mesh_rot` the
/// mesh's bind (`poseToBone` inverted), `pose_gap` (units) and `pose_turn` (degrees) how far they are apart.
unsafe fn model_skeleton_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let s = read_mdl(unsafe { api.check_bytes(l, 1)? })?;
    unsafe {
        (api.createtable)(l, 0, 4);
        api.set_str(l, c"name", &s.name);
        api.set_number(l, c"version", f64::from(s.version));
        api.set_number(l, c"checksum", f64::from(s.checksum));
        (api.createtable)(l, s.bones.len() as c_int, 0);
        for (i, b) in s.bones.iter().enumerate() {
            (api.createtable)(l, 0, 9);
            api.set_str(l, c"name", &b.name);
            api.set_number(l, c"parent", b.parent.map_or(-1.0, |p| p as f64));
            api.set_vector(l, c"pos", &b.bind.0.to_array());
            api.set_vector(l, c"rot", &b.bind.1.to_array());
            api.set_number(l, c"physics", f64::from(b.physics));
            api.set_vector(l, c"mesh_pos", &b.pose.0.to_array());
            api.set_vector(l, c"mesh_rot", &b.pose.1.to_array());
            api.set_number(l, c"pose_gap", f64::from(b.bind.0.distance(b.pose.0)));
            api.set_number(l, c"pose_turn", f64::from(b.bind.1.angle_between(b.pose.1).to_degrees()));
            api.append(l, i + 1);
        }
        (api.setfield)(l, -2, c"bones".as_ptr());
    }
    Ok(1)
}

/// A mapping handed over by Lua (`{[part] = bone}`, NM's part names; `nil` for the rig's own), as `{part, bone}`.
unsafe fn mapping_arg(api: &Api, l: *mut LuaState, arg: c_int) -> Result<Option<Vec<(String, String)>>, String> {
    let t = unsafe { (api.type_)(l, arg) };
    if t <= lua::TNIL {
        return Ok(None);
    }
    if t != TTABLE {
        return Err(format!("bad argument #{arg} (a mapping {{[part] = bone}} expected)"));
    }
    let mut out = vec![];
    for part in rigs::PARTS {
        let name = std::ffi::CString::new(part).unwrap_or_default();
        unsafe { (api.getfield)(l, arg, name.as_ptr()) };
        if unsafe { (api.type_)(l, -1) } > lua::TNIL {
            out.push((part.to_string(), unsafe { api.check_string(l, -1) }?));
        }
        unsafe { (api.settop)(l, -2) };
    }
    Ok(Some(out))
}

/// A vector at absolute stack index `at`: `{x, y, z}` or `{x =, y =, z =}`; `None` for nil.
unsafe fn vec_at(api: &Api, l: *mut LuaState, at: c_int) -> Result<Option<Vec3>, String> {
    if unsafe { (api.type_)(l, at) } <= lua::TNIL {
        return Ok(None);
    }
    unsafe { api.check_vec3(l, at) }.map(|v| Some(Vec3::from_array(v)))
}

/// A number field of the table at absolute index `t`, `None` for nil.
unsafe fn num_field(api: &Api, l: *mut LuaState, t: c_int, name: &CStr) -> Result<Option<f32>, String> {
    unsafe {
        (api.getfield)(l, t, name.as_ptr());
        let top = (api.gettop)(l);
        let v = if (api.type_)(l, top) <= lua::TNIL { Ok(None) } else { api.check_number(l, top).map(|n| Some(n as f32)) };
        (api.settop)(l, top - 1);
        v
    }
}

/// A vector field of the table at absolute index `t`, `None` for nil.
unsafe fn vec_field(api: &Api, l: *mut LuaState, t: c_int, name: &CStr) -> Result<Option<Vec3>, String> {
    unsafe {
        (api.getfield)(l, t, name.as_ptr());
        let top = (api.gettop)(l);
        let v = vec_at(api, l, top);
        (api.settop)(l, top - 1);
        v
    }
}

/// The player's edits handed over by Lua (`nil` for none): `{joints = {[part] = {x, y, z}} (units), parts = {[part] =
/// {radius, half (m), box = {x, y, z} (m), mass (kg), offset = {x, y, z} (m), turn = {p, y, r} (degrees)}}}`.
unsafe fn edits_arg(api: &Api, l: *mut LuaState, arg: c_int) -> Result<Option<rigs::Edits>, String> {
    let t = unsafe { (api.type_)(l, arg) };
    if t <= lua::TNIL {
        return Ok(None);
    }
    if t != TTABLE {
        return Err(format!("bad argument #{arg} (edits {{joints, parts}} expected)"));
    }
    let mut e = rigs::Edits::default();
    let bad = |what: &str, part: &str, err: String| format!("bad argument #{arg} ({what}.{part}: {err})");
    unsafe {
        (api.getfield)(l, arg, c"joints".as_ptr());
        let joints = (api.gettop)(l);
        if (api.type_)(l, joints) == TTABLE {
            for (k, part) in rigs::PARTS.iter().enumerate() {
                let name = std::ffi::CString::new(*part).unwrap_or_default();
                if let Some(v) = vec_field(api, l, joints, &name).map_err(|err| bad("joints", part, err))? {
                    e.joints[k] = v;
                }
            }
        }
        (api.settop)(l, joints - 1);
        (api.getfield)(l, arg, c"parts".as_ptr());
        let parts = (api.gettop)(l);
        if (api.type_)(l, parts) == TTABLE {
            for (k, part) in rigs::PARTS.iter().enumerate() {
                let name = std::ffi::CString::new(*part).unwrap_or_default();
                (api.getfield)(l, parts, name.as_ptr());
                let p = (api.gettop)(l);
                if (api.type_)(l, p) == TTABLE {
                    let r = (|| -> Result<rigs::PartEdit, String> {
                        Ok(rigs::PartEdit {
                            radius: num_field(api, l, p, c"radius")?,
                            half: num_field(api, l, p, c"half")?,
                            boxed: vec_field(api, l, p, c"box")?,
                            mass: num_field(api, l, p, c"mass")?,
                            offset: vec_field(api, l, p, c"offset")?.unwrap_or(Vec3::ZERO),
                            turn: vec_field(api, l, p, c"turn")?.unwrap_or(Vec3::ZERO),
                        })
                    })();
                    match r {
                        Ok(pe) => e.parts[k] = pe,
                        Err(err) => {
                            (api.settop)(l, parts - 1);
                            return Err(bad("parts", part, err));
                        }
                    }
                }
                (api.settop)(l, p - 1);
            }
        }
        (api.settop)(l, parts - 1);
    }
    Ok(Some(e))
}

/// `{[part] = bone}` of a mapping (`nil` parts left out).
unsafe fn push_map(api: &Api, l: *mut LuaState, map: &[Option<String>; 21]) {
    unsafe {
        (api.createtable)(l, 0, 21);
        for (k, b) in map.iter().enumerate() {
            if let Some(b) = b {
                api.set_str(l, &std::ffi::CString::new(rigs::PARTS[k]).unwrap_or_default(), b);
            }
        }
    }
}

unsafe fn push_strings(api: &Api, l: *mut LuaState, items: &[String]) {
    unsafe {
        (api.createtable)(l, items.len() as c_int, 0);
        for (i, w) in items.iter().enumerate() {
            api.push_str(l, w);
            api.append(l, i + 1);
        }
    }
}

native!(model_classify => model_classify_impl);
/// `DMR.ModelClassify(mdl[, mapping])`: what the `.mdl`'s skeleton is to DMR (ROADMAP M12): `{rig, complete,
/// humanoid, why, map = {[part] = bone}, missing = {parts}, problems = {...}, report = {...}, parts = {NM's 21 part
/// names}, optional = {[part] = true}}`; `rig` a known rig's name, `by names`, `mapped` (with `mapping`), `unknown` or
/// `not humanoid`; `report` the reader's (a broken bind pose).
unsafe fn model_classify_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let s = read_mdl(unsafe { api.check_bytes(l, 1)? })?;
    let names = unsafe { mapping_arg(api, l, 2)? };
    let at: Vec<Vec3> = s.bones.iter().map(|b| b.bind.0).collect();
    let class = rigs::classify(&s, names.as_deref(), &at)?;
    let (_, report) = bind_report(&s, &class.map);
    let map = std::array::from_fn(|k| class.map[k].map(|b| s.bones[b].name.clone()));
    let missing: Vec<String> = class.missing.iter().map(|p| p.to_string()).collect();
    unsafe {
        (api.createtable)(l, 0, 10);
        api.set_str(l, c"rig", &class.rig);
        api.set_bool(l, c"complete", class.complete());
        api.set_bool(l, c"humanoid", class.humanoid());
        api.set_str(l, c"why", &if class.complete() { String::new() } else { class.why(&s.name) });
        push_map(api, l, &map);
        (api.setfield)(l, -2, c"map".as_ptr());
        push_strings(api, l, &missing);
        (api.setfield)(l, -2, c"missing".as_ptr());
        push_strings(api, l, &class.problems);
        (api.setfield)(l, -2, c"problems".as_ptr());
        push_strings(api, l, &report);
        (api.setfield)(l, -2, c"report".as_ptr());
        push_strings(api, l, &rigs::PARTS.map(String::from));
        (api.setfield)(l, -2, c"parts".as_ptr());
        (api.createtable)(l, 0, rigs::OPTIONAL.len() as c_int);
        for k in rigs::OPTIONAL {
            api.set_bool(l, &std::ffi::CString::new(rigs::PARTS[k]).unwrap_or_default(), true);
        }
        (api.setfield)(l, -2, c"optional".as_ptr());
    }
    Ok(1)
}

/// A fit's numbers for Lua: height, mass, rig, warnings, the mapping it used.
unsafe fn push_fit_summary(api: &Api, l: *mut LuaState, f: &Fitted) {
    unsafe {
        api.set_number(l, c"height", f64::from(f.height));
        api.set_number(l, c"mass", f64::from(f.mass));
        api.set_str(l, c"rig", &f.rig);
        push_strings(api, l, &f.warnings);
        (api.setfield)(l, -2, c"warnings".as_ptr());
        push_map(api, l, &f.map);
        (api.setfield)(l, -2, c"map".as_ptr());
    }
}

native!(ragdoll_forget => ragdoll_forget_impl);
/// `DMR.RagdollForget(model)`: the model's kept fit dropped (its mapping changed); bodies already made keep theirs.
unsafe fn ragdoll_forget_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let key = unsafe { api.check_string(l, 1)? };
    if let Some(m) = FITS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        m.remove(&key);
    }
    Ok(0)
}

native!(ragdoll_fit => ragdoll_fit_impl);
/// `DMR.RagdollFit(model, mdl[, mapping[, edits]])`: Homer's build fitted to the humanoid `.mdl` (`model` its path, `mdl` its
/// bytes) by its rig or `mapping` (`{[part] = bone}`, ROADMAP M12), kept by the path, the `.mdl`'s checksum and the
/// mapping and edits (`edits_arg`; a changed model, mapping or edits is fitted again): `{height, mass, rig, map, homer_height, homer_mass, checksum,
/// cached, ms, warnings = {...}, bones = {[bone] = part}, finish = {{bone, parent}, ...}, parts = {{name, mass,
/// radius, half, size}, ...}}` (m, kg; `bones` the model's bones NM's parts pose, 0-based parts; `finish` the bones the
/// client puts right). A Lua error for a model that can't be fitted (not a humanoid, an unknown rig without a mapping).
unsafe fn ragdoll_fit_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let key = unsafe { api.check_string(l, 1)? };
    let bytes = unsafe { api.check_bytes(l, 2)? };
    let names = unsafe { mapping_arg(api, l, 3)? };
    let edits = unsafe { edits_arg(api, l, 4)? };
    let d = data::get().ok_or_else(|| format!("NM's data isn't loaded (DMR.LoadData): {}", data::how_to()))?;
    let started = std::time::Instant::now();
    let checksum = i32_at(bytes, 8).unwrap_or(0);
    let kept = fitted(&key).ok().filter(|f| f.checksum == checksum && f.names == names && f.edits == edits);
    let cached = kept.is_some();
    let f = match kept {
        Some(f) => f,
        None => {
            let f = Arc::new(fit(&d, &read_mdl(bytes)?, &key, names.as_deref(), edits.as_ref())?);
            FITS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new).insert(key, f.clone());
            f
        }
    };
    let body = &f.def.body;
    unsafe {
        (api.createtable)(l, 0, 14);
        push_fit_summary(api, l, &f);
        (api.createtable)(l, f.finish.len().min(FINISH_MAX) as c_int, 0);
        for (i, (bone, parent)) in f.finish.iter().take(FINISH_MAX).enumerate() {
            (api.createtable)(l, 2, 0);
            api.push_str(l, bone);
            api.append(l, 1);
            api.push_str(l, parent);
            api.append(l, 2);
            api.append(l, i + 1);
        }
        (api.setfield)(l, -2, c"finish".as_ptr());
        api.set_number(l, c"homer_height", f64::from(height(&d.body)));
        api.set_number(l, c"homer_mass", f64::from(d.body.parts.iter().map(|p| p.mass).sum::<f32>()));
        api.set_number(l, c"checksum", f64::from(f.checksum));
        api.set_bool(l, c"cached", cached);
        api.set_number(l, c"ms", started.elapsed().as_secs_f64() * 1000.0);
        (api.createtable)(l, 0, f.posed.len() as c_int);
        for b in &f.posed {
            api.set_number(l, &std::ffi::CString::new(b.bone.as_str()).unwrap_or_default(), b.part as f64);
        }
        (api.setfield)(l, -2, c"bones".as_ptr());
        (api.createtable)(l, body.parts.len() as c_int, 0);
        for (k, p) in body.parts.iter().enumerate() {
            (api.createtable)(l, 0, 5);
            api.set_str(l, c"name", p.name);
            api.set_number(l, c"mass", f64::from(p.mass));
            api.set_number(l, c"radius", f64::from(p.radius));
            api.set_number(l, c"half", f64::from(p.half));
            api.set_number(l, c"size", f64::from(f.def.build.parts[k].size));
            api.append(l, k + 1);
        }
        (api.setfield)(l, -2, c"parts".as_ptr());
    }
    Ok(1)
}

native!(fit_dump => fit_dump_impl);
/// `DMR.FitDump(model)`: the fitted build as the Playroom's `NM_DUMP` writes one (`Build::dump`).
unsafe fn fit_dump_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    unsafe { api.push_str(l, &f.def.build.dump(&f.frames)) };
    Ok(1)
}

native!(fit_joints => fit_joints_impl);
/// `DMR.FitJoints(model)`: the 21 joints Homer's build was fitted to (`Build::fit`'s input, as `nm_build_fit` takes
/// them), a line `x y z` (m) each, for `golden body --fit` to fit Homer's build itself: the dump can't carry a fitted
/// build (its sizes).
unsafe fn fit_joints_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let lines: Vec<String> = f.joints.iter().map(|j| format!("{:.9} {:.9} {:.9}", j.x, j.y, j.z)).collect();
    unsafe { api.push_str(l, &lines.join("\n")) };
    Ok(1)
}

native!(fit_lead => fit_lead_impl);
/// `DMR.FitLead(model, steps[, message])`: the fitted body in LEAD (NM's own physics, the crate as the Playroom runs
/// it) on the Playroom's floor, `bodyBalance` from the start (or `message`): `{z = {pelvis height per step}, fell}`
/// (m), whether it stands without Box3D at all.
unsafe fn fit_lead_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let steps = unsafe { api.check_number(l, 2)? } as usize;
    let line = if unsafe { (api.type_)(l, 3) } > lua::TNIL { unsafe { api.check_string(l, 3)? } } else { "bodyBalance start=true".into() };
    let d = data::get().ok_or("NM's data isn't loaded")?;
    let z = lead(&d, &f.def.build, &f.def.body, steps, &line)?;
    let low = z.iter().copied().fold(f32::MAX, f32::min);
    unsafe {
        (api.createtable)(l, 0, 2);
        (api.createtable)(l, z.len() as c_int, 0);
        for (i, v) in z.iter().enumerate() {
            (api.pushnumber)(l, f64::from(*v));
            api.append(l, i + 1);
        }
        (api.setfield)(l, -2, c"z".as_ptr());
        api.set_bool(l, c"fell", low < z.first().copied().unwrap_or(0.0) * 0.6);
    }
    Ok(1)
}

/// A LEAD agent on `build` as the Playroom makes one (its room's floor, seed 0.3, `msg::send`), stepped `steps`
/// times at 1/60 s: the pelvis' height each step.
pub fn lead(d: &data::Data, build: &Build, body: &dms_game::body::Body, steps: usize, line: &str) -> Result<Vec<f32>, String> {
    let placed: Vec<Transform> = body.placed(Vec3::ZERO, 0.0).into_iter().map(|(p, q)| Transform::from_translation(p).with_rotation(q)).collect();
    let world = data::floor_collision(d, [0.0, 0.0, 0.0], false);
    let mut agent = delirium::nm::Agent::new(build, Ragdoll::new(build, &placed, Vec3::ZERO), Quat::IDENTITY, 0.3);
    let mut msgs = delirium::msg::State::default();
    let mut words = line.split_whitespace();
    let name = words.next().ok_or("no message")?;
    let ps: Vec<(&str, delirium::msg::Value)> = words
        .filter_map(|w| w.split_once('='))
        .map(|(k, v)| (k, if v == "true" { delirium::msg::Value::Bool(true) } else { delirium::msg::Value::Float(v.parse().unwrap_or(0.0)) }))
        .collect();
    delirium::msg::send(&mut agent, build, &world, &mut msgs, name, &ps).map_err(|e| format!("{name}: {e:?}"))?;
    let mut z = Vec::with_capacity(steps);
    for _ in 0..steps {
        agent.step(build, &world, 1.0 / 60.0, &[], &[]);
        z.push(agent.doll.frames().next().map_or(0.0, |f| f.translation.z));
    }
    Ok(z)
}

/// Source's angles (pitch, yaw, roll; degrees) of a rotation (`MatrixAngles`).
pub fn angles(q: Quat) -> [f32; 3] {
    let m = Mat3::from_quat(q);
    let (fwd, left, up) = (m.x_axis, m.y_axis, m.z_axis);
    let xy = (fwd.x * fwd.x + fwd.y * fwd.y).sqrt();
    let (pitch, yaw, roll) = if xy > 0.001 {
        ((-fwd.z).atan2(xy), fwd.y.atan2(fwd.x), left.z.atan2(up.z))
    } else {
        ((-fwd.z).atan2(xy), (-left.x).atan2(left.y), 0.0)
    };
    [pitch.to_degrees(), yaw.to_degrees(), roll.to_degrees()]
}

/// The game flag that keeps an entity's objects off each other (Source's `FVPHYSICS_NO_SELF_COLLISIONS`): the native
/// bodies take their owner's entity, so the game keeps them off the model's own objects; among themselves the fork's
/// pairs (`SetBodyCollisionPairs`) decide first.
const NO_SELF_COLLISIONS: u16 = 0x8000;

static NATIVE: Mutex<Option<(u32, HashMap<u32, NativeBody>)>> = Mutex::new(None);

fn native(handle: u32) -> Result<NativeBody, String> {
    NATIVE.lock().unwrap_or_else(|e| e.into_inner()).as_ref().and_then(|(_, m)| m.get(&handle).cloned()).ok_or_else(|| format!("no native body #{handle}"))
}

/// The model's ragdoll objects handed over by Lua: `{{name = bone, obj = PhysObj, pos = its origin (units)}, ...}`.
unsafe fn model_objects(api: &Api, l: *mut LuaState, arg: c_int) -> Result<Vec<(String, *mut IPhysicsObject, Vec3)>, String> {
    if unsafe { (api.type_)(l, arg) } != TTABLE {
        return Err(format!("bad argument #{arg} (a table of {{name, obj, pos}} expected)"));
    }
    let mut out = vec![];
    for i in 1.. {
        unsafe { (api.rawgeti)(l, arg, i) };
        if unsafe { (api.type_)(l, -1) } != TTABLE {
            unsafe { (api.settop)(l, -2) };
            break;
        }
        let top = unsafe { (api.gettop)(l) };
        let item = (|| unsafe {
            (api.getfield)(l, top, c"name".as_ptr());
            let name = api.check_string(l, -1);
            (api.settop)(l, -2);
            (api.getfield)(l, top, c"obj".as_ptr());
            let obj = api.check_physobj(l, -1);
            (api.settop)(l, -2);
            (api.getfield)(l, top, c"pos".as_ptr());
            let pos = api.check_vec3(l, -1);
            (api.settop)(l, -2);
            Ok::<_, String>((name?, obj?, Vec3::from_array(pos?) * M))
        })();
        unsafe { (api.settop)(l, -2) };
        out.push(item.map_err(|e| format!("bad argument #{arg} ([{i}]: {e})"))?);
    }
    Ok(out)
}

/// Every part's state from the model's ragdoll as it is now: the parts whose bones have an object from them (each
/// bone's bind-pose offset to its part), the rest between their neighbours ([`BETWEEN`]), or off their parent as in
/// the bind pose.
fn parts_from_model(f: &Fitted, objects: &[(String, *mut IPhysicsObject, Vec3)]) -> Result<Vec<PartState>, String> {
    let nm = vphysics_nm::get().ok_or("IPhysicsNM isn't available")?;
    let n = f.def.body.parts.len();
    let mut parts: Vec<Option<PartState>> = vec![None; n];
    // A part's state from an object of a bone it poses.
    let from = |p: &Posed, obj: *mut IPhysicsObject, origin: Vec3| -> Result<PartState, String> {
        // SAFETY: a PhysObj of the model's ragdoll Lua handed over.
        let s = unsafe { nm.body_state(obj) }.ok_or("GetBodyState failed")?;
        let bone_rot = Quat::from_array(s.rotation).normalize();
        let rot = (bone_rot * p.turn.inverse()).normalize();
        let pos = origin - rot * p.at;
        let (com, v, w) = (Vec3::from_array(s.position), Vec3::from_array(s.velocity), Vec3::from_array(s.angular_velocity));
        Ok(PartState { pos, rot, vel: v + w.cross(pos - com), spin: w })
    };
    let posed = |name: &str, own: bool| f.posed.iter().find(|p| p.own == own && p.bone.eq_ignore_ascii_case(name));
    for (name, obj, origin) in objects {
        if let Some(p) = posed(name, true) {
            parts[p.part] = Some(from(p, *obj, *origin)?);
        }
    }
    // The root without a bone object of its own: from a physics bone it carries (a rig whose root object is a bone
    // above the pelvis).
    for (name, obj, origin) in objects {
        if let Some(p) = posed(name, false).filter(|p| parts[p.part].is_none() && p.part == 0) {
            parts[0] = Some(from(p, *obj, *origin)?);
        }
    }
    if parts[0].is_none() {
        return Err("the model's ragdoll has no pelvis object".into());
    }
    // Part b's frame as it is in the bind pose relative to part a's, carried by a's state.
    let rel = |a: &PartState, ka: usize, kb: usize| -> PartState {
        let (fa, fb) = (f.frames[ka], f.frames[kb]);
        let rot = (a.rot * fa.rotation.inverse() * fb.rotation).normalize();
        let pos = a.pos + a.rot * (fa.rotation.inverse() * (fb.translation - fa.translation));
        PartState { pos, rot, vel: a.at(pos), spin: a.spin }
    };
    for &(k, a, b, t) in &BETWEEN {
        if parts[k].is_some() {
            continue;
        }
        if let (Some(pa), Some(pb)) = (parts[a], parts[b]) {
            let (x, y) = (rel(&pa, a, k), rel(&pb, b, k));
            parts[k] = Some(PartState { pos: x.pos.lerp(y.pos, t), rot: x.rot.slerp(y.rot, t).normalize(), vel: x.vel.lerp(y.vel, t), spin: x.spin.lerp(y.spin, t) });
        }
    }
    // Parts with no own object nor neighbours on both sides: from the physics bones they carry.
    for (name, obj, origin) in objects {
        if let Some(p) = posed(name, false).filter(|p| parts[p.part].is_none()) {
            parts[p.part] = Some(from(p, *obj, *origin)?);
        }
    }
    for k in 0..n {
        if parts[k].is_none() {
            let p = f.def.body.parts[k].parent.ok_or("a root without an object")?;
            parts[k] = parts[p].map(|pp| rel(&pp, p, k));
        }
    }
    parts.into_iter().collect::<Option<Vec<_>>>().ok_or_else(|| "a part's pose couldn't be made".into())
}

native!(ragdoll_body => ragdoll_body_impl);
/// `DMR.RagdollBody(model, owner, objects | {pos =, yaw =})`: the fitted body made by the fork in the environment of
/// `owner` (a PhysObj of the model's ragdoll, whose entity the bodies stand in for): NM's capsules and boxes as
/// `dms_vphysics::hull` hulls them (the fork keeps the hulls), NM's masses, a ragdoll constraint per joint, bound to NM's
/// body (pivots, frames, limits, inertia, materials) and only NM's self-collision pairs colliding (`VBOX3D_NM_006`).
/// Posed as the model's ragdoll is now and moving as its objects do (`objects`: `{{name = bone, obj = PhysObj, pos =
/// its origin}, ...}`, as `DMR.PoseModel` gives them back), or standing as `nm_agent_create` puts one at `pos` (units,
/// its feet's level) facing `yaw` (degrees). Its handle and how long it took (ms).
unsafe fn ragdoll_body_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let owner = unsafe { api.check_physobj(l, 2)? };
    let nm = vphysics_nm::get().ok_or("IPhysicsNM isn't available")?;
    if nm.version < Version::V006 {
        return Err(format!("{} can't make GMod's ragdolls' bodies: update VPhysics-Box3D ({} needed)", nm.version.name(), Version::V006.name()));
    }
    let body = &f.def.body;
    // `{pos, yaw}` (a table with a `pos`) or the objects' list.
    unsafe { (api.getfield)(l, 3, c"pos".as_ptr()) };
    let stand = unsafe { (api.type_)(l, -1) } > lua::TNIL;
    unsafe { (api.settop)(l, -2) };
    let states: Vec<PartState> = if stand {
        unsafe { (api.getfield)(l, 3, c"pos".as_ptr()) };
        let at = unsafe { api.check_vec3(l, -1) };
        unsafe { (api.settop)(l, -2) };
        unsafe { (api.getfield)(l, 3, c"yaw".as_ptr()) };
        let yaw = unsafe { api.opt_number(l, -1, 0.0) };
        unsafe { (api.settop)(l, -2) };
        body.placed(Vec3::from_array(at?) * M, (yaw? as f32).to_radians())
            .into_iter()
            .map(|(pos, rot)| PartState { pos, rot, vel: Vec3::ZERO, spin: Vec3::ZERO })
            .collect()
    } else {
        parts_from_model(&f, &unsafe { model_objects(api, l, 3)? })?
    };
    if states.iter().any(|s| !(s.pos.is_finite() && s.rot.is_finite() && s.vel.is_finite() && s.spin.is_finite())) {
        return Err("the ragdoll's pose isn't finite".into());
    }
    let started = std::time::Instant::now();
    // SAFETY: a PhysObj Lua handed over; its live environment, outside the step callback (Lua).
    let made = unsafe { NativeBody::make(&nm, nm.object_environment(owner), owner, body, &states, NO_SELF_COLLISIONS)? };
    let ms = started.elapsed().as_secs_f64() * 1000.0;
    let mut slot = NATIVE.lock().unwrap_or_else(|e| e.into_inner());
    let (next, map) = slot.get_or_insert_with(|| (0, HashMap::new()));
    *next += 1;
    map.insert(*next, made);
    unsafe {
        (api.pushnumber)(l, f64::from(*next));
        (api.pushnumber)(l, ms);
    }
    Ok(2)
}

native!(ragdoll_warm => ragdoll_warm_impl);
/// `DMR.RagdollWarm(model, owner, part)`: the fork's hull of the fitted body's part `part` (0-based) made ahead of the
/// body (a body made and gone at once; the fork keeps the hull, `CreateBody`), so the first body of a model doesn't
/// make all 21 in one frame (ROADMAP M11: a death). How long it took (ms); nothing for a part past the last.
unsafe fn ragdoll_warm_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let owner = unsafe { api.check_physobj(l, 2)? };
    let k = unsafe { api.check_number(l, 3)? } as usize;
    let nm = vphysics_nm::get().ok_or("IPhysicsNM isn't available")?;
    if nm.version < Version::V006 {
        return Err(format!("{} can't make GMod's ragdolls' bodies", nm.version.name()));
    }
    let Some(p) = f.def.body.parts.get(k) else { return Ok(0) };
    let started = std::time::Instant::now();
    let points: Vec<[f32; 3]> = dms_vphysics::hull::points(&p.shape).iter().map(|v| v.to_array()).collect();
    // SAFETY: a PhysObj Lua handed over; outside the step callback (Lua). The body goes before any step.
    unsafe {
        let env = nm.object_environment(owner);
        let s = nm.body_state(owner).ok_or("GetBodyState failed")?;
        if let Some(o) = nm.create_body(env, owner, &points, p.mass, &s.position, &[0.0, 0.0, 0.0, 1.0], NO_SELF_COLLISIONS).filter(|o| !o.is_null()) {
            nm.destroy_body(o);
        }
        (api.pushnumber)(l, started.elapsed().as_secs_f64() * 1000.0);
    }
    Ok(1)
}

native!(ragdoll_follow => ragdoll_follow_impl);
/// `DMR.RagdollFollow(model, handle, yaw, floor)`: NM on a native body (`DMR.Follow` for a fitted one): `yaw` where
/// it faces (rad), `floor` (m) where NM's frame starts. The agent's id; `DMR.Unfollow` it before `DMR.RagdollRemove`.
unsafe fn ragdoll_follow_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let n = native(unsafe { api.check_number(l, 2)? } as u32)?;
    let yaw = unsafe { api.check_number(l, 3)? } as f32;
    let floor = Vec3::from_array(unsafe { api.check_vec3(l, 4)? });
    // SAFETY: Lua unfollows before it removes the native body.
    let id = unsafe { crate::agents::start_body(n.objects, f.def.clone(), yaw, floor, false)? };
    unsafe { (api.pushnumber)(l, f64::from(id)) };
    Ok(1)
}

/// The native body `handle`'s objects (the bench's fitted runs, `dev.rs`).
pub fn native_objects(handle: u32) -> Result<Vec<*mut IPhysicsObject>, String> {
    native(handle).map(|n| n.objects)
}

/// A fitted body's definition (the bench's fitted runs).
pub fn fitted_def(model: &str) -> Result<Arc<BodyDef>, String> {
    fitted(model).map(|f| f.def.clone())
}

native!(ragdoll_report => ragdoll_report_impl);
/// `DMR.RagdollReport(model, handle)`: the native body as the game has it, for `golden body --fit` (as
/// `DMR_HomerReport` has Homer's): a `part <name> <centre x y z> <rotation x y z w> <centre again> <mass> <inertia's
/// diagonal> <damping lin ang> <tensor, 9>` line a part (m, kg, kg m^2; no bone of its own, so its centre stands for
/// the origin), then `joints <found by FindConstraint> <objects>`.
unsafe fn ragdoll_report_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let n = native(unsafe { api.check_number(l, 2)? } as u32)?;
    let nm = vphysics_nm::get().ok_or("IPhysicsNM isn't available")?;
    let body = &f.def.body;
    let mut out = vec![];
    for (p, &o) in body.parts.iter().zip(&n.objects) {
        // SAFETY: the native body's live objects, on the main thread (Lua).
        let s = unsafe { nm.body_state(o) }.ok_or("GetBodyState failed")?;
        let t = unsafe { nm.body_inertia(o) }.unwrap_or([0.0; 9]);
        let (mass, [lin, ang]) = unsafe { (vphysics_nm::object::mass(o), vphysics_nm::object::damping(o)) };
        let [x, y, z] = s.position;
        let [qx, qy, qz, qw] = s.rotation;
        let tensor: Vec<String> = t.iter().map(|v| format!("{v:.9}")).collect();
        out.push(format!(
            "part {} {x:.9} {y:.9} {z:.9} {qx:.9} {qy:.9} {qz:.9} {qw:.9} {x:.9} {y:.9} {z:.9} {mass:.9} {:.9} {:.9} {:.9} {lin:.9} {ang:.9} {}",
            p.name, t[0], t[4], t[8], tensor.join(" ")
        ));
    }
    // SAFETY: as above.
    let joints = body.joints.iter().filter(|j| !unsafe { nm.find_constraint(n.objects[j.parent], n.objects[j.part]) }.is_null()).count();
    out.push(format!("joints {joints} {}", n.objects.len()));
    unsafe { api.push_str(l, &out.join("\n")) };
    Ok(1)
}

native!(ragdoll_remove => ragdoll_remove_impl);
/// `DMR.RagdollRemove(handle)`: the native body's joints and bodies gone (its agent taken off first, `DMR.Unfollow`).
unsafe fn ragdoll_remove_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let handle = unsafe { api.check_number(l, 1)? } as u32;
    let n = NATIVE.lock().unwrap_or_else(|e| e.into_inner()).as_mut().and_then(|(_, m)| m.remove(&handle));
    if let (Some(n), Some(nm)) = (n, vphysics_nm::get()) {
        // SAFETY: made by `ragdoll_body`, not destroyed yet.
        unsafe { n.destroy(&nm) };
    }
    Ok(0)
}

native!(ragdoll_shapes => ragdoll_shapes_impl);
/// `DMR.RagdollShapes(model)`: the fitted body's parts' shapes in their frames, for drawing NM's body over a ragdoll
/// (the mapping editor's world view): `{{name, shape = "capsule", a, b, radius} | {name, shape = "box", ang, half},
/// ...}` (units; `ang` Source's angles in the part's frame).
unsafe fn ragdoll_shapes_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    unsafe {
        (api.createtable)(l, 21, 0);
        for (k, p) in f.def.body.parts.iter().enumerate() {
            (api.createtable)(l, 0, 5);
            api.set_str(l, c"name", p.name);
            match p.shape {
                dms_game::body::Shape::Capsule { rot, radius, half } => {
                    let axis = rot * Vec3::Y * half;
                    api.set_str(l, c"shape", "capsule");
                    api.set_vector(l, c"a", &(-axis / M).to_array());
                    api.set_vector(l, c"b", &(axis / M).to_array());
                    api.set_number(l, c"radius", f64::from(radius / M));
                }
                dms_game::body::Shape::Box { rot, half } => {
                    api.set_str(l, c"shape", "box");
                    api.set_vector(l, c"ang", &angles(rot));
                    api.set_vector(l, c"half", &(half / M).to_array());
                }
            }
            api.append(l, k + 1);
        }
    }
    Ok(1)
}

native!(ragdoll_parts => ragdoll_parts_impl);
/// `DMR.RagdollParts(handle)`: the native body's parts as they are now, a flat `{x, y, z, pitch, yaw, roll, ...}` per
/// part in part order (units, degrees), for drawing NM's body over the ragdoll.
unsafe fn ragdoll_parts_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let parts = native(unsafe { api.check_number(l, 1)? } as u32)?.objects;
    let nm = vphysics_nm::get().ok_or("IPhysicsNM isn't available")?;
    unsafe { (api.createtable)(l, parts.len() as c_int * 6, 0) };
    let mut i = 0;
    for &o in &parts {
        // SAFETY: the native body's live objects, on the main thread.
        let s = unsafe { nm.body_state(o) }.ok_or("GetBodyState failed")?;
        let [x, y, z] = (Vec3::from_array(s.position) / M).to_array();
        let [p, w, r] = angles(Quat::from_array(s.rotation).normalize());
        for v in [x, y, z, p, w, r] {
            i += 1;
            unsafe {
                (api.pushnumber)(l, f64::from(v));
                api.append(l, i);
            }
        }
    }
    Ok(1)
}

native!(pose_model => pose_model_impl);
/// `DMR.PoseModel(model, handle)`: where the model's mapped bones are with the native body's parts as they are now
/// (S4): `{[bone name] = {pos (units), ang = {x = pitch, y = yaw, z = roll}, rot (quaternion), vel (m/s, the bone's
/// origin's), spin (rad/s, world)}}`, for posing the model every tick and for handing the motion back on detach.
unsafe fn pose_model_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    let f = fitted(&unsafe { api.check_string(l, 1)? })?;
    let parts = native(unsafe { api.check_number(l, 2)? } as u32)?.objects;
    let nm = vphysics_nm::get().ok_or("IPhysicsNM isn't available")?;
    let states = parts
        .iter()
        // SAFETY: the native body's live objects.
        .map(|&o| unsafe { nm.body_state(o) }.ok_or("GetBodyState failed"))
        .collect::<Result<Vec<_>, _>>()?;
    unsafe { (api.createtable)(l, 0, f.posed.len() as c_int) };
    for b in &f.posed {
        let s = states[b.part];
        let (p, q) = (Vec3::from_array(s.position), Quat::from_array(s.rotation).normalize());
        let (v, w) = (Vec3::from_array(s.velocity), Vec3::from_array(s.angular_velocity));
        let (pos, rot) = (p + q * b.at, (q * b.turn).normalize());
        let [pitch, yaw, roll] = angles(rot);
        let name = std::ffi::CString::new(b.bone.as_str()).unwrap_or_default();
        unsafe {
            (api.createtable)(l, 0, 5);
            api.set_vector(l, c"pos", &(pos / M).to_array());
            api.set_vector(l, c"ang", &[pitch, yaw, roll]);
            api.set_vector(l, c"rot", &rot.to_array());
            api.set_vector(l, c"vel", &(v + w.cross(pos - p)).to_array());
            api.set_vector(l, c"spin", &w.to_array());
            (api.setfield)(l, -2, name.as_ptr());
        }
    }
    Ok(1)
}
