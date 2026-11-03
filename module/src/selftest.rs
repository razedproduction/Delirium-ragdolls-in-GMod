//! `dms_selftest`: Homer's agent in LEAD on the Playroom's floor, `bodyBalance` (the Playroom's "balance") for
//! a couple of seconds — he has to stay up (ROADMAP M2). No game physics is involved: NM moves its own body.
//!
//! The agent is the crate's, made as the Playroom makes its own (`Agent::new` on Homer's build, `msg::send`), not the
//! C API's: since SDK v0.5 a build read back from a dump (`nm_build_from_text`) has no links (`Build::links`; the
//! dump doesn't carry them), so the C API's Homer stands in another pose than the Playroom's (docs/adr/0016).

use delirium::prelude::*;
use delirium::ragdoll::Ragdoll;

use crate::data::{self, Data};
use crate::nm;

/// What came of it.
pub struct Report {
    pub steps: usize,
    pub pelvis_start: f32,
    pub pelvis_min: f32,
    pub pelvis_end: f32,
    pub head_start: f32,
    pub head_end: f32,
    pub balancing: bool,
    pub fallen: bool,
    pub passed: bool,
    pub verdict: String,
}

/// Homer as the Playroom starts him (`nm_agent_create` at the origin, yaw 0, seed 0.3: `Body::placed`), on its room's
/// floor with the pool, after `bodyBalance start=true`.
pub fn playroom_agent(d: &Data) -> Result<(delirium::nm::Agent, delirium::collision::Collision), String> {
    let world = data::floor_collision(d, [0.0; 3], true);
    let placed: Vec<Transform> = d.body.placed(Vec3::ZERO, 0.0).into_iter().map(|(p, q)| Transform::from_translation(p).with_rotation(q)).collect();
    let mut agent = delirium::nm::Agent::new(&d.homer, Ragdoll::new(&d.homer, &placed, Vec3::ZERO), Quat::IDENTITY, nm::PLAYROOM_SEED);
    let mut msgs = delirium::msg::State::default();
    delirium::msg::send(&mut agent, &d.homer, &world, &mut msgs, "bodyBalance", &[("start", delirium::msg::Value::Bool(true))])
        .map_err(|e| format!("bodyBalance: {e:?}"))?;
    Ok((agent, world))
}

/// Runs `seconds` of NM steps (1/60 s).
pub fn run(d: &Data, seconds: f32) -> Result<Report, String> {
    if !(seconds > 0.0 && seconds <= 600.0) {
        return Err("seconds: 0 to 600".into());
    }
    let head = (0..nm::PARTS).find(|&i| nm::part_name(i).is_ok_and(|n| n == "Head")).ok_or("no part Head")?;
    let (mut a, world) = playroom_agent(d)?;
    let frames = |a: &delirium::nm::Agent| -> Vec<Transform> { a.doll.frames().collect() };
    let start = frames(&a);
    let (pelvis_start, head_start) = (start[0].translation.z, start[head].translation.z);
    let (mut pelvis_min, mut finite) = (pelvis_start, true);
    let steps = (seconds / nm::DT).round() as usize;
    let mut now = start;
    for _ in 0..steps {
        a.step(&d.homer, &world, nm::DT, &[], &[]);
        now = frames(&a);
        finite &= now.iter().all(|f| f.translation.is_finite() && f.rotation.is_finite());
        if !finite {
            break;
        }
        pelvis_min = pelvis_min.min(now[0].translation.z);
    }
    let (pelvis_end, head_end) = (now[0].translation.z, now[head].translation.z);
    let (balancing, fallen) = (a.balancing(), a.fallen);
    // Standing: still balancing, not fallen, the pelvis and head close to their standing heights.
    let checks = [
        (finite, "NaN/Inf in the body"),
        (!fallen, "NM says he has fallen"),
        (balancing, "he isn't balancing"),
        (pelvis_min > 0.75 * pelvis_start, "the pelvis sank below 3/4 of its height"),
        (head_end > 0.85 * head_start, "the head dropped below 85% of its height"),
    ];
    let failed: Vec<&str> = checks.iter().filter(|c| !c.0).map(|c| c.1).collect();
    let passed = failed.is_empty();
    Ok(Report {
        steps,
        pelvis_start,
        pelvis_min,
        pelvis_end,
        head_start,
        head_end,
        balancing,
        fallen,
        passed,
        verdict: if passed { "PASS: he stands".into() } else { format!("FAIL: {}", failed.join("; ")) },
    })
}
