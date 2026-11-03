//! The Playroom room's shot (`fire`, `shot_frame`; docs/adr/0016, 0033): a bullet's damage on his health (the game's
//! `ped::damage` with weaponinfo.xml's weapon), the limb's injury mask, the bullet's impulse and the ShotTask with the
//! weapon's taskparams set, then its messages every other step. The module's room (`module/src/follow/room.rs`) and
//! the rig editor's agent shoot through it, so a shot is the same in both.
//!
//! The state it changes is the caller's (the Playroom `Sim`'s fields, [`Victim`]); what it says and sends comes back
//! as text and message lines for the caller's `say` and `send`, in the order the Playroom's `fire` has them.

use delirium::nm::{self, Agent, Hit};
use delirium::prelude::*;
use delirium::shottask::{self, ShotTask};
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;
use delirium_data::weapons::Weapon;

use super::{PARTS, v3};

/// The part a shot goes into without one given (the Playroom's `shot` button).
pub const CHEST: usize = 9;
const THIGH_L: usize = 1;
const THIGH_R: usize = 4;
pub const START_HEALTH: f32 = 200.0;
const INJURY_MAX: f32 = 0.8;
/// The Playroom's guns: its name, weaponinfo.xml's, the force without the file.
pub const WEAPONS: [(&str, &str, f32); 13] = [
    ("pistol", "PISTOL", 50.0),
    ("deagle", "DEAGLE", 50.0),
    ("smg", "MP5", 50.0),
    ("ak47", "AK47", 50.0),
    ("sniper", "SNIPERRIFLE", 50.0),
    ("shotgun", "SHOTGUN", 50.0),
    ("rpg", "RLAUNCHER", 50.0),
    ("grenade", "GRENADE", 50.0),
    ("molotov", "MOLOTOV", 50.0),
    ("bat", "BASEBALLBAT", 50.0),
    ("knife", "KNIFE", 50.0),
    ("taser", "TASER", 10.0),
    ("baton", "BASEBALLBAT", 50.0),
];

/// Which of the Playroom's weapons a name is (its own or weaponinfo.xml's), pistol for an unknown one.
pub fn weapon(name: &str) -> usize {
    WEAPONS.iter().position(|w| w.0.eq_ignore_ascii_case(name) || w.1.eq_ignore_ascii_case(name)).unwrap_or(0)
}

/// His forward from his parts' places (the room's `Side`: from the left thigh to the right, turned).
pub fn forward(parts: &[Vec3]) -> Vec3 {
    let right = parts.get(THIGH_R).copied().unwrap_or(Vec3::X) - parts.get(THIGH_L).copied().unwrap_or(Vec3::ZERO);
    Vec3::Z.cross(right).with_z(0.0).normalize_or(Vec3::Y)
}

/// The Playroom's `now` (ms) after `steps` NM steps.
pub fn now(steps: u64) -> u32 {
    (steps as f64 * 1000.0 / 60.0) as u32
}

/// The Playroom's `injure`: a limb's injury mask, worse with each hit, as its line (none for a part off the limbs).
pub fn injury(hurt: &mut [f32; 4], part: usize, by: f32) -> Option<String> {
    let l = [(13..=16, 0), (17..=20, 1), (1..=3, 2), (4..=6, 3)].into_iter().find(|r| r.0.contains(&part)).map(|r| r.1)?;
    hurt[l] = (hurt[l] + by).min(INJURY_MAX);
    Some(format!("injury mask={} amount={:.2}", ["ul", "ur", "ll", "lr"][l], hurt[l]))
}

/// The room's state a shot reads and changes.
pub struct Victim<'a> {
    pub health: &'a mut f32,
    pub rand: &'a mut Rand,
    pub hurt: &'a mut [f32; 4],
    /// The ShotTask running, and the step it was made on.
    pub task: &'a mut Option<(ShotTask, u64)>,
}

/// A shot: into `part`, `aim` the hit point, the bullet's way and the surface's normal (None: from his front, 10 cm
/// out), a kill, the shotgun's other pellets' parts (their damage added), the Playroom's weapon; `weak` (agility 2 for
/// the weapon's set) and `god` (health kept) are the room's switches.
pub struct Shot<'a> {
    pub part: usize,
    pub aim: Option<(Vec3, Vec3, Vec3)>,
    pub fatal: bool,
    pub more: &'a [usize],
    pub weapon: usize,
    pub weak: bool,
    pub god: bool,
}

/// What a shot did: the room's line about it, where it hit (NM's frame), the lines to send him in order.
pub struct Fired {
    pub say: String,
    pub at: Vec3,
    pub lines: Vec<String>,
}

/// The Playroom's `fire`: `s` into `agent`'s body after `steps` NM steps; `dead` he was dead before it (only the
/// impulse then). `weapons` are weaponinfo.xml's (none: the impulse at the fallback force, the default set, no
/// damage), `tp` the taskparams the ShotTask reads.
pub fn fire(v: Victim, agent: &Agent, weapons: &[Weapon], tp: &TaskParams, s: &Shot, dead: bool, steps: u64) -> Fired {
    let part = s.part.min(PARTS - 1);
    let was_dead = dead || *v.health <= 0.0;
    let (label, name, fallback) = WEAPONS[s.weapon.min(WEAPONS.len() - 1)];
    let weapon = weapons.iter().find(|x| x.name == name);
    let hit = weapon.map(|wp| {
        use delirium_data::ped;
        let mut h = ped::Health { health: *v.health, ..Default::default() };
        let (mut lost, mut instant) = (0.0, false);
        for &p in std::iter::once(&part).chain(s.more) {
            let hit = ped::Hit::new(wp.damage, wp, delirium_data::nm_asset::PART_BONES[p], ped::Attacker::Player { sure: true });
            let o = ped::damage(&mut h, &hit, &ped::Victim::default(), v.rand);
            (lost, instant) = (lost + o.lost, instant || o.flags & ped::INSTANT() != 0);
        }
        *v.health = h.health;
        (lost, instant)
    });
    if s.fatal {
        *v.health = 0.0;
    }
    if s.god {
        *v.health = START_HEALTH;
    }
    let health = *v.health;
    let state = if health < 50.0 { "fatally injured" } else if health < 100.0 { "injured (dying)" } else { "standing" };
    let say = format!(
        "shot ({label}) at part {part} {}: -{:.1}{} -> health {health:.1} ({state})",
        delirium_data::nm_asset::PARTS.get(part).unwrap_or(&"?"),
        hit.map_or(0.0, |h| h.0),
        if hit.is_some_and(|h| h.1) { " (instant kill)" } else { "" },
    );
    let (at, d, n) = s.aim.unwrap_or_else(|| {
        let parts: Vec<Vec3> = agent.doll.frames().map(|f| f.translation).collect();
        let fwd = forward(&parts);
        (parts.get(part).copied().unwrap_or(Vec3::ZERO) + fwd * 0.1, -fwd, fwd)
    });
    let mut lines = vec![];
    if let Some((lost, _)) = hit
        && !was_dead
    {
        lines.extend(injury(v.hurt, part, (lost / 100.0).clamp(0.15, 0.5)));
    }
    let agility = if s.weak { 2 } else { 3 };
    let set = weapon.map_or("*", |wp| nm::weapon_set(wp, agility));
    let vel = shottask::bullet(at, at - d.normalize() * 3.0, weapon.map_or(fallback, |wp| wp.force), false);
    lines.push(format!("applyImpulse partIndex={part} hitPoint={} impulse={}", v3(at), v3(vel)));
    if was_dead {
        return Fired { say, at, lines };
    }
    let hit = Hit { health, ..Hit::ragdoll(agent) };
    match v.task.as_mut() {
        Some((t, _)) => lines.extend(t.next_hit(v.rand, tp, &hit, now(steps), set, part, at, vel, n).iter().map(shottask::text)),
        None => *v.task = Some((ShotTask::new(&hit, set, part, at, vel, n), steps)),
    }
    Fired { say, at, lines }
}

/// One of the ShotTask's frames (the Playroom's `shot_frame`, every other step, from the second step after it was
/// made): its message lines, whether it fired back (no gun here), whether it finished (and was dropped).
pub struct TaskStep {
    pub lines: Vec<String>,
    pub fire: bool,
    pub done: bool,
}

/// The ShotTask's frame after `steps` NM steps, his `health` now; None while there's none due.
pub fn task_frame(task: &mut Option<(ShotTask, u64)>, rand: &mut Rand, tp: &TaskParams, agent: &Agent, health: f32, steps: u64) -> Option<TaskStep> {
    let (mut t, made) = match task.take() {
        Some((t, made)) if steps >= made + 2 => (t, made),
        other => {
            *task = other;
            return None;
        }
    };
    let hit = Hit { health, ..Hit::ragdoll(agent) };
    let step = t.update(rand, tp, &hit, now(steps));
    if !step.done {
        *task = Some((t, made));
    }
    Some(TaskStep { lines: step.msgs.iter().map(shottask::text).collect(), fire: step.fire, done: step.done })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injury_and_weapons_as_the_room_has_them() {
        let mut hurt = [0.0; 4];
        assert_eq!(injury(&mut hurt, 14, 0.5).as_deref(), Some("injury mask=ul amount=0.50"));
        assert_eq!(injury(&mut hurt, 14, 0.5).as_deref(), Some("injury mask=ul amount=0.80"));
        assert_eq!(injury(&mut hurt, CHEST, 0.5), None);
        assert_eq!((weapon("AK47"), weapon("sniperrifle"), weapon("nothing")), (3, 4, 0));
        let parts = [Vec3::ZERO, Vec3::new(-0.1, 0.0, 0.0), Vec3::ZERO, Vec3::ZERO, Vec3::new(0.1, 0.0, 0.0)];
        assert!(forward(&parts).abs_diff_eq(Vec3::Y, 1e-6));
        assert_eq!(now(60), 1000);
    }
}
