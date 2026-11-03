//! NM's FOLLOW step on the fork's bodies (ROADMAP M4, docs/adr/0008, 0016): the pieces of it that only read and
//! write the crate `delirium` (the Playroom's agent, its message lines, the balancer's impulses, the drives and
//! armature out of NM's doll, gravity's compensation, docs/adr/0007). The module's agents (`module/src/follow.rs`,
//! with the Playroom room's reactions round them) and the rig editor's (`tools/dms-rig`, bodyBalance on a fitted body)
//! are made of the same pieces, so a body is driven the same in both.
//!
//! Nothing here calls the fork: the callers read the bodies (`GetBodyState`) and apply what comes out
//! (`ApplyBodyDrive`, `SetJointArmature`) from their step callbacks.

use delirium::msg::{self, Kind, Message, Value};
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::{Build, GRAVITY, Ragdoll};

use crate::nm::PhysicsNMBodyState;

pub mod shot;

pub const PARTS: usize = 21;
/// NM's own step.
pub const DT: f64 = 1.0 / 60.0;
/// The Playroom's agents' seed (`Agent::new(.., 0.3)`).
pub const SEED: f32 = 0.3;
/// Past these a part has blown up (m/s, rad/s): NM's physics caps a part at 80 m/s (`ragdoll::MAX_SPEED`), and no
/// motion of a body turns a part at 300 rad/s.
pub const MAX_SPEED: f32 = 80.0;
pub const MAX_SPIN: f32 = 300.0;
/// The Playroom's grab (`pull`): its spring and its cap (N s a step at 60 steps a second).
pub const PULL_K: f32 = 1500.0;
pub const PULL_MAX: f32 = 1500.0;
/// The Playroom's click (`poke`, N s) and the part it goes into without one given.
pub const POKE: f32 = 8.0;
pub const POKE_PART: usize = 9;

/// The Playroom's agent on a body standing at `frames` (NM's frame), facing `yaw` (rad): `Agent::new(build,
/// Ragdoll::new(build, frames, 0), facing, 0.3)`.
pub fn agent(build: &Build, frames: &[Transform], yaw: f32) -> Agent {
    Agent::new(build, Ragdoll::new(build, frames, Vec3::ZERO), Quat::from_rotation_z(yaw), SEED)
}

/// The Playroom's buttons that only send message lines (`Sim::lines`), the ones the bench presses.
pub fn button_lines(b: &str) -> Option<&'static [&'static str]> {
    Some(match b {
        "balance" => &["bodyBalance start=true"],
        "windmill" => &["bodyBalance start=true", "armsWindmillAdaptive start=true"],
        "pedal" => &["pedalLegs start=true"],
        "writhe" => &["bodyWrithe start=true"],
        "rollup" => &["bodyRollUp start=true"],
        "catchfall" => &["catchFall start=true"],
        "highfall" => &["highFall start=true"],
        "stairs" => &["rollDownStairs start=true"],
        "relax" => &["bodyRelax start=true"],
        "stayup" => &["stayUpright start=true stiffness=3"],
        "foetal" => &["bodyFoetal start=true"],
        "STOP all" => &["stopAllBehaviours"],
        _ => return None,
    })
}

/// The Playroom's `v3`: a vector in a message line.
pub fn v3(v: Vec3) -> String {
    format!("{:.3},{:.3},{:.3}", v.x, v.y, v.z)
}

/// The Playroom's `poke`: `impulse` N s into part `k` at its centre `at` (NM's frame) along `dir`, as its line.
pub fn poke_line(k: usize, at: Vec3, dir: Vec3, impulse: f32) -> String {
    format!("applyImpulse start=true partIndex={k} hitPoint={} impulse={}", v3(at), v3(dir.normalize() * impulse))
}

/// The Playroom's `parse`: a param's text as a value of its kind.
pub fn parse_value(kind: Kind, v: &str) -> Option<Value> {
    Some(match kind {
        Kind::Float => Value::Float(v.parse().ok()?),
        Kind::Int => Value::Int(v.parse().ok()?),
        Kind::Bool => Value::Bool(match v {
            "true" | "1" => true,
            "false" | "0" => false,
            _ => return None,
        }),
        Kind::Vec3 => {
            let f: Vec<f32> = v.split(',').map(|x| x.parse().ok()).collect::<Option<_>>()?;
            Value::Vec3(Vec3::new(*f.first()?, *f.get(1)?, *f.get(2)?))
        }
        Kind::Str => Value::Str(v.to_string()),
    })
}

/// A message line (`<message> [param=value ...]`) as the Playroom's console parses it (`send_to`): its message and
/// params, or what's wrong with it (the console's words); `None` for an empty line.
pub fn parse_line(line: &str) -> Option<Result<(&'static Message, Vec<(&str, Value)>), String>> {
    let mut w = line.split_whitespace();
    let name = w.next()?;
    let Some(m) = msg::find(name) else { return Some(Err(format!("no message {name}"))) };
    let mut ps: Vec<(&str, Value)> = vec![];
    for kv in w {
        let Some((k, v)) = kv.split_once('=') else { return Some(Err(format!("{kv}: want name=value"))) };
        let kind = if k == "start" { Some(Kind::Bool) } else { m.params.iter().find(|p| p.name == k).map(|p| p.kind) };
        let Some(kind) = kind else { return Some(Err(format!("{name} has no param {k}"))) };
        match parse_value(kind, v) {
            Some(v) => ps.push((k, v)),
            None => return Some(Err(format!("{k}: not a {kind:?}"))),
        }
    }
    Some(Ok((m, ps)))
}

/// `applyImpulse`'s part, impulse, hit point and equalising from its params, the message's defaults for the rest
/// (as `msg::send` takes them: an int or a float for the part, a float or an int for the amount).
pub fn impulse_params(ps: &[(&str, Value)]) -> (usize, Vec3, Option<Vec3>, f32) {
    let get = |n: &str| ps.iter().find(|(p, _)| *p == n).map(|(_, v)| v.clone()).or_else(|| msg::get(&msg::State::default(), "applyImpulse", n));
    let num = |v: Option<Value>| match v {
        Some(Value::Float(x)) => Some(x),
        Some(Value::Int(i)) => Some(i as f32),
        _ => None,
    };
    let vec = |v: Option<Value>| match v {
        Some(Value::Vec3(v)) => Some(v),
        _ => None,
    };
    let part = num(get("partIndex")).unwrap_or(0.0).max(0.0) as usize;
    let hit = ps.iter().any(|(p, _)| *p == "hitPoint").then(|| vec(get("hitPoint"))).flatten();
    (part.min(PARTS - 1), vec(get("impulse")).unwrap_or(Vec3::ZERO), hit, num(get("equalizeAmount")).unwrap_or(0.0))
}

/// `applyImpulse` moves the Playroom's doll (`equalized_impulse`, solved in its physics step); in FOLLOW the body is
/// the fork's, so the same impulse goes on it: part `k`, the point from its centre, the impulse (N s). None if it
/// isn't finite.
pub fn kick(doll: &Ragdoll, k: usize, impulse: Vec3, hit: Option<Vec3>, equalize: f32) -> Option<(usize, Vec3, Vec3)> {
    let centre = doll.frames().nth(k)?.translation;
    let mean = doll.mass() / PARTS as f32;
    let s = 1.0 - equalize + equalize * doll.part_mass(k) / mean;
    let (at, j) = (hit.unwrap_or(centre), impulse * s);
    (at.is_finite() && j.is_finite()).then_some((k, at - centre, j))
}

/// The Playroom's `pull` (its mouse grab): a spring on part `part`'s point `local` (its frame) toward `target` (NM's
/// frame), as `applyImpulse`'s params of this step, damped and capped as there. None if it isn't finite.
pub fn pull_params(doll: &Ragdoll, part: usize, local: Vec3, target: Vec3) -> Option<[(&'static str, Value); 4]> {
    let f = doll.frames().nth(part)?;
    let at = f.translation + f.rotation * local;
    let (v, w) = doll.vel(part);
    let damp = (2.0 * (PULL_K * doll.mass()).sqrt()).min(30.0 * doll.part_mass(part));
    let imp = (PULL_K * (target - at) - damp * (v + w.cross(at - f.translation))).clamp_length_max(PULL_MAX) / 60.0;
    if !imp.is_finite() {
        return None;
    }
    Some([("start", Value::Bool(true)), ("partIndex", Value::Int(part as i32)), ("hitPoint", Value::Vec3(at)), ("impulse", Value::Vec3(imp))])
}

/// Every part's state from the fork (`snap`, the game's frame) into NM's doll (`set_body`), NM's frame at `origin`:
/// an error for a state that isn't finite or has blown up. How much the body moves: the root's speed and spin
/// squared, and the parts' mean of both, for keeping NM's sleep ([`keep_sleep`]).
pub fn set_state(doll: &mut Ragdoll, snap: &[PhysicsNMBodyState], origin: Vec3) -> Result<(f32, f32, f32), String> {
    let mut moved = (0.0f32, 0.0f32, 0.0f32);
    for (k, s) in snap.iter().enumerate().take(PARTS) {
        let (x, q) = (Vec3::from_array(s.position) - origin, Quat::from_array(s.rotation));
        let (v, w) = (Vec3::from_array(s.velocity), Vec3::from_array(s.angular_velocity));
        if !(x.is_finite() && q.is_finite() && v.is_finite() && w.is_finite()) || q.length_squared() < 0.5 {
            return Err(format!("part {k}'s state isn't finite"));
        }
        // A blow-up is as bad as a NaN: faster than NM's own physics ever lets a part go.
        if v.length() > MAX_SPEED || w.length() > MAX_SPIN {
            return Err(format!("part {k} blew up ({:.0} m/s, {:.0} rad/s)", v.length(), w.length()));
        }
        if k == 0 {
            (moved.0, moved.1) = (v.length_squared(), w.length_squared());
        }
        moved.2 += (v.length_squared() + w.length_squared()) / PARTS as f32;
        doll.set_body(k, x, q, v, w);
    }
    Ok(moved)
}

/// `set_body` wakes the doll; the Playroom's only wakes when it moves (`Ragdoll::sleep`'s tolerances): its mode and
/// sleep from before [`set_state`] kept unless the body `moved` past them.
pub fn keep_sleep(doll: &mut Ragdoll, (mode, asleep): (u8, bool), moved: (f32, f32, f32)) {
    let [v2, w2, mean2] = doll.sleep_tol;
    if moved.0 <= v2 && moved.1 <= w2 && moved.2 <= mean2 {
        (doll.mode, doll.asleep) = (mode, asleep);
    }
}

/// NM's balancer steadies the body with impulses (`nmdbm::stabilise` while `bodyBalance` runs: the feet one way, the
/// pelvis the other), which its step hands to its own physics (`doll.impulse`), not to `doll.out`: in FOLLOW they'd
/// be lost. The same function on the same state (the step's, before it runs) gives them at a unit gain; once the step
/// has set its gain, [`balancer_kicks`] scales them.
pub fn balancer_unit(agent: &Agent, build: &Build, dt: f32) -> Option<Vec<Vec3>> {
    agent.character().map(|ch| {
        let frames: Vec<Transform> = agent.doll.frames().collect();
        let body = delirium::nmdbm::Body { ch, build, doll: &agent.doll, frames: &frames };
        delirium::nmdbm::stabilise(&body, 1.0, dt)
    })
}

/// [`balancer_unit`]'s impulses at the gain the step set (`Character::stabilise`; they're linear in its square), as
/// kicks at the parts' centres: part, arm, impulse (N s).
pub fn balancer_kicks(agent: &Agent, unit: Option<Vec<Vec3>>) -> Vec<(usize, Vec3, Vec3)> {
    let (Some(unit), Some((gain, _))) = (unit, agent.character().and_then(|ch| ch.stabilise)) else { return vec![] };
    unit.into_iter().enumerate().filter(|(_, j)| *j != Vec3::ZERO).map(|(k, j)| (k, Vec3::ZERO, j * (gain * gain))).collect()
}

/// The step's drives (`doll.out`, gravity left out) and armature (`doll.armature`, by joint: the root's unused)
/// copied out of NM's doll, checked.
pub fn outputs(doll: &Ragdoll, drives: &mut [(Vec3, Vec3)], armature: &mut [f32]) -> Result<(), String> {
    if doll.out.len() < PARTS || doll.out.iter().any(|(f, t)| !(f.is_finite() && t.is_finite())) {
        return Err("NM's drives aren't finite".into());
    }
    drives.copy_from_slice(&doll.out[..PARTS]);
    for k in 1..PARTS {
        let a = doll.armature.get(k).copied().unwrap_or(0.0);
        if !(a.is_finite() && a >= 0.0) {
            return Err(format!("joint {k}'s armature isn't finite"));
        }
        armature[k] = a;
    }
    Ok(())
}

/// The forces and torques of one physics step of `timestep` s, by part (N, N m, world): NM's `drives` (none while
/// `asleep`), gravity's compensation (docs/adr/0007: `m_k (g_nm - g_src)` on each part the game pulls, `comp` of it;
/// `gravity_off` the parts it doesn't, by bit) and the `kicks` (part, arm, impulse) spread over the step.
#[allow(clippy::too_many_arguments)]
pub fn forces(
    doll: &Ragdoll,
    drives: &[(Vec3, Vec3)],
    asleep: bool,
    comp: f32,
    g_src: Vec3,
    gravity_off: u32,
    kicks: impl IntoIterator<Item = (usize, Vec3, Vec3)>,
    timestep: f32,
) -> Vec<(Vec3, Vec3)> {
    let g_nm = Vec3::NEG_Z * GRAVITY();
    let mut forces: Vec<(Vec3, Vec3)> = if asleep { vec![(Vec3::ZERO, Vec3::ZERO); PARTS] } else { drives.to_vec() };
    for (k, f) in forces.iter_mut().enumerate().filter(|_| !asleep) {
        let g = if gravity_off >> k & 1 != 0 { Vec3::ZERO } else { g_src };
        f.0 += (g_nm - g) * (doll.part_mass(k) * comp);
    }
    for (k, arm, j) in kicks {
        let f = j / timestep;
        forces[k].0 += f;
        forces[k].1 += arm.cross(f);
    }
    forces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_parse_as_the_console_does() {
        for l in button_lines("balance").unwrap() {
            let (m, ps) = parse_line(l).unwrap().unwrap();
            assert_eq!((m.name, ps.len()), ("bodyBalance", 1));
        }
        assert!(parse_line("  ").is_none());
        let err = |l: &str| parse_line(l).and_then(Result::err);
        assert_eq!(err("noSuchMessage").as_deref(), Some("no message noSuchMessage"));
        assert_eq!(err("bodyBalance start").as_deref(), Some("start: want name=value"));
        assert_eq!(err("bodyBalance start=maybe").as_deref(), Some("start: not a Bool"));
        let poke = poke_line(POKE_PART, Vec3::new(0.0, 0.0, 1.0), Vec3::X, POKE);
        let (_, ps) = parse_line(&poke).unwrap().unwrap();
        assert_eq!(impulse_params(&ps), (POKE_PART, Vec3::new(POKE, 0.0, 0.0), Some(Vec3::new(0.0, 0.0, 1.0)), 0.0));
    }
}
