//! The fitted body on the fork itself (ROADMAP M12.5 step 1, docs/adr/0031): `vphysics.dll` (VPhysics-Box3D) loaded
//! into the editor, an environment as the game's server sets one up (0.015 s steps, air, a concrete floor with NM's
//! material), the body made natively as the module makes it (`dms_vphysics::body::NativeBody::make`), then dropped,
//! thrown and dragged by the mouse.
//!
//! With NM on it (docs/adr/0032), the Playroom's agent drives the body in FOLLOW as the module's agents do: a step
//! callback on the environment (`SetStepCallback`) reads the parts (`GetBodyState`), runs NM's 1/60 s steps due on
//! an accumulator, and puts NM's armature on the joints and its drives, the balancer's impulses and gravity's
//! compensation on the bodies (`ApplyBodyDrive`), all of it `dms_vphysics::follow`'s, the module's own pieces. Of the
//! Playroom room's reactions round the agent only its shot is here (`fire`, `shot_frame`, health and death:
//! `dms_vphysics::follow::shot`, the module's too, docs/adr/0033); its tasks, get-ups, `in_air` and the additional
//! reactions are the module's and aren't: the editor runs the behaviour it's given and nothing more.

use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use delirium::collision::Collision;
use delirium::msg;
use delirium::nm::Agent;
use delirium::prelude::*;
use delirium::ragdoll::Build;
use delirium::shottask::ShotTask;
use delirium_data::rng::Rand;
use delirium_data::taskparams::TaskParams;
use delirium_data::weapons::Weapon;
use dms_fit::M;
use dms_game::body::Body;
use dms_vphysics::body::{NativeBody, PartState};
use dms_vphysics::follow::shot::{self, CHEST, START_HEALTH};
use dms_vphysics::follow::{self as core, PARTS};
use dms_vphysics::host::{Environment, Host};
use dms_vphysics::nm::{self, IPhysicsConstraint, IPhysicsEnvironment, IPhysicsObject, Nm, PhysicsNMBodyState};

/// The game's physics step (`IPhysicsEnvironment::SetSimulationTimestep` on the server, whatever the tickrate).
pub const TIMESTEP: f32 = 0.015;
/// Source's `sv_gravity` (units/s^2), and NM's (m/s^2) that the module makes the body feel under NM (docs/adr/0007).
pub const SOURCE_GRAVITY: f32 = 600.0;
/// (A function since SDK v0.2.6: it's one of the numbers read from the game, `delirium_data::kv`.)
pub fn nm_gravity() -> f32 {
    delirium::ragdoll::GRAVITY()
}
/// NM steps one physics step may run at most, the rest of the time dropped (the module's `MAX_STEPS`).
const MAX_STEPS: u32 = 4;
/// The agent's lines kept for the panel.
const LOG: usize = 12;

/// Where the fork's `vphysics.dll` is: Garry's Mod's (installed by `scripts/install.ps1 -Fork`), else the
/// repository's build. Its imports (`tier0.dll`) come from Garry's Mod's `bin\win64` either way.
pub fn find_vphysics(gmod: &Path, repo: Option<&Path>) -> Vec<PathBuf> {
    let mut v = vec![gmod.join("bin").join("win64").join("vphysics.dll")];
    if let Some(r) = repo {
        for p in ["vphysics/build/windows-x64/src/Release/vphysics.dll", "vphysics/build/windows-x64/Release/vphysics.dll", "vphysics/build/src/Release/vphysics.dll"] {
            v.push(r.join(p));
        }
    }
    v.into_iter().filter(|p| p.is_file()).collect()
}

/// What the room's shot reads from the player's GTA IV, as the module loads it (`Data`): weaponinfo.xml's weapons
/// and taskparams.txt (NM's defaults without one).
#[derive(Default)]
pub struct GameData {
    pub weapons: Vec<Weapon>,
    pub tp: TaskParams,
}

/// A shot asked for: into `part` with the Playroom's weapon `weapon`, from the front (its `shot` button) or from behind
/// (`shot back`).
#[derive(Clone, Copy, Debug)]
pub struct ShotAt {
    pub part: usize,
    pub weapon: usize,
    pub back: bool,
}

impl Default for ShotAt {
    fn default() -> ShotAt {
        ShotAt { part: CHEST, weapon: 0, back: false }
    }
}

/// The room's state the shot keeps (the module's `Room` fields `fire` and `shot_frame` read), for an agent without
/// the rest of the room; its switches as the Playroom's defaults (`weak`, `god` off).
pub struct Shots {
    data: Arc<GameData>,
    pub health: f32,
    pub dead: bool,
    rand: Rand,
    hurt: [f32; 4],
    task: Option<(ShotTask, u64)>,
}

impl Shots {
    pub fn new(data: Arc<GameData>) -> Shots {
        Shots { data, health: START_HEALTH, dead: false, rand: Rand::default(), hurt: [0.0; 4], task: None }
    }

    /// The ShotTask is running.
    pub fn task(&self) -> bool {
        self.task.is_some()
    }

    /// The room's `fire` (`shot` and `shot back` among its buttons) on `agent` after `steps` NM steps: what to say
    /// and the lines to send him.
    pub fn fire(&mut self, agent: &Agent, at: ShotAt, steps: u64) -> shot::Fired {
        let aim = at.back.then(|| {
            let parts: Vec<Vec3> = agent.doll.frames().map(|f| f.translation).collect();
            let fwd = shot::forward(&parts);
            (parts.get(at.part).copied().unwrap_or(Vec3::ZERO) - fwd * 0.1, fwd, -fwd)
        });
        let s = shot::Shot { part: at.part.min(PARTS - 1), aim, fatal: false, more: &[], weapon: at.weapon, weak: false, god: false };
        let v = shot::Victim { health: &mut self.health, rand: &mut self.rand, hurt: &mut self.hurt, task: &mut self.task };
        shot::fire(v, agent, &self.data.weapons, &self.data.tp, &s, self.dead, steps)
    }

    /// The room's `shot_frame` (every other step): death at health 0 (`STOP all`, then `relax`), the ShotTask's
    /// lines; what to say and the lines to send.
    pub fn frame(&mut self, agent: &Agent, steps: u64) -> (Vec<String>, Vec<String>) {
        let (mut say, mut lines) = (vec![], vec![]);
        if self.health <= 0.0 && !self.dead {
            self.dead = true;
            self.task = None;
            for b in ["STOP all", "relax"] {
                lines.extend(core::button_lines(b).unwrap_or_default().iter().map(|l| l.to_string()));
            }
            say.push("dead (health 0): limp".to_string());
        }
        if let Some(step) = shot::task_frame(&mut self.task, &mut self.rand, &self.data.tp, agent, self.health, steps) {
            lines.extend(step.lines);
            if step.fire {
                say.push("blind fire shot (no gun here)".into());
            }
            if step.done {
                say.push("the shot task finished".into());
            }
        }
        (say, lines)
    }
}

/// A part held by the mouse: the point on it (m, the part's frame) and where it's pulled to (m, world).
pub struct Grab {
    pub part: usize,
    pub local: Vec3,
    pub target: Vec3,
}

/// The Playroom's agent on the body, stepped from the environment's step callback (the module's `Follower` without
/// its room). NM's frame is the world's: the floor at z = 0 under the body's spawn.
pub struct NmAgent {
    build: Build,
    world: Collision,
    pub agent: Agent,
    msgs: msg::State,
    objects: Vec<*mut IPhysicsObject>,
    joints: Vec<*mut IPhysicsConstraint>,
    /// NM steps run, the time towards the next and those due this physics step.
    pub steps: u64,
    acc: f64,
    due: u32,
    snap: Vec<PhysicsNMBodyState>,
    /// The last step's drives, held until the next (docs/adr/0008); the armature NM wants and the joints have.
    drives: Vec<(Vec3, Vec3)>,
    armature: Vec<f32>,
    armature_set: Vec<f32>,
    /// `applyImpulse`'s and the balancer's impulses for the next physics step: part, arm, impulse.
    kicks: Vec<(usize, Vec3, Vec3)>,
    queued: Vec<String>,
    /// The Playroom's grab: part, the point in its frame, the target.
    pub grab: Option<(usize, Vec3, Vec3)>,
    /// Gravity's compensation on (docs/adr/0007, `dms_gravity_mode nm`) or off (`off`), and Source's gravity (m/s^2).
    pub comp: f32,
    g_src: Vec3,
    /// What it said (its messages and what they found wrong), the newest last.
    pub log: Vec<String>,
    /// Why NM came off the body.
    pub failed: Option<String>,
    /// What one NM step costs (ms, smoothed).
    pub cost: f64,
    /// The Playroom's button it was started with.
    pub button: String,
    /// NM's feedback of the last step.
    feedback: Vec<String>,
    /// The room's shot on him, and the shots asked for the next step.
    pub shots: Shots,
    shoot: Vec<ShotAt>,
}

impl NmAgent {
    /// NM's agent on `made` (the body `build` drives) as it stands now, facing `yaw` (rad), on NM's floor of
    /// `floor_material` (the Playroom's: CONCRETE's 1 m squares, 50 m either way).
    ///
    /// # Safety
    /// `made`'s bodies are live until the agent is dropped.
    unsafe fn new(nm: &Nm, made: &NativeBody, build: &Build, data: Arc<GameData>, floor_material: u8, yaw: f32, comp: bool) -> Result<NmAgent, String> {
        // SAFETY: the caller's.
        let states = unsafe { made.states(nm)? };
        let frames: Vec<Transform> = states.iter().map(|s| Transform::from_translation(s.pos).with_rotation(s.rot)).collect();
        let (p, t) = dms_game::floor(false, 50.0);
        let mut world = Collision::default();
        world.add(&p, &t, &vec![floor_material; t.len()]);
        Ok(NmAgent {
            build: build.clone(),
            world,
            agent: core::agent(build, &frames, yaw),
            msgs: msg::State::default(),
            objects: made.objects.clone(),
            joints: made.joints.clone(),
            steps: 0,
            // The first physics step runs a step: NM drives the body from the start.
            acc: core::DT,
            due: 0,
            snap: vec![],
            drives: vec![(Vec3::ZERO, Vec3::ZERO); PARTS],
            armature: vec![0.0; PARTS],
            armature_set: vec![0.0; PARTS],
            kicks: vec![],
            queued: vec![],
            grab: None,
            comp: if comp { 1.0 } else { 0.0 },
            g_src: Vec3::new(0.0, 0.0, -SOURCE_GRAVITY * M),
            log: vec![],
            failed: None,
            cost: 0.0,
            button: String::new(),
            feedback: vec![],
            shots: Shots::new(data),
            shoot: vec![],
        })
    }

    /// A message line for the next NM step (`<message> [param=value ...]`, the Playroom's console's).
    pub fn queue(&mut self, line: &str) {
        self.queued.push(line.trim().to_string());
    }

    /// A shot for the next NM step (the room's `fire`, before the step as the module's reactions are).
    pub fn shoot(&mut self, at: ShotAt) {
        self.shoot.push(at);
    }

    /// NM's doll is asleep (the Playroom's `mode` 2): no behaviours, no drives.
    fn asleep(&self) -> bool {
        self.agent.doll.mode == 2 && self.agent.doll.asleep
    }

    fn say(&mut self, s: String) {
        self.log.push(s);
        let n = self.log.len();
        if n > LOG {
            self.log.drain(..n - LOG);
        }
    }

    /// The module's `send` (the Playroom's `send_to`), without the tuning profile (docs/adr/0009: off, as in the
    /// Playroom): parsed as its console does, through `msg::send`; `applyImpulse` goes on the bodies too.
    fn send(&mut self, line: &str) {
        let (m, ps) = match core::parse_line(line) {
            None => return,
            Some(Err(e)) => return self.say(e),
            Some(Ok(x)) => x,
        };
        let kick = (m.name == "applyImpulse").then(|| core::impulse_params(&ps));
        match msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, m.name, &ps) {
            Ok(_) => self.say(format!("MSG {}", line.chars().take(110).collect::<String>())),
            Err(e) => self.say(format!("{}: {e:?}", m.name)),
        }
        if let Some((k, impulse, hit, equalize)) = kick {
            self.kicks.extend(core::kick(&self.agent.doll, k, impulse, hit, equalize));
        }
    }

    /// One NM step (the module's `nm_step` on the state `begin` read, the room's parts left out).
    fn nm_step(&mut self) -> Result<(), String> {
        let dt = core::DT as f32;
        let sleep = (self.agent.doll.mode, self.agent.doll.asleep);
        let moved = core::set_state(&mut self.agent.doll, &self.snap, Vec3::ZERO)?;
        core::keep_sleep(&mut self.agent.doll, sleep, moved);
        for line in std::mem::take(&mut self.queued) {
            self.send(&line);
        }
        // The Playroom's `tick`: the outbox (`extras_frame`), the grab (`pull`), then the step.
        for l in std::mem::take(&mut self.agent.extras.outbox) {
            match l.strip_prefix("#say ") {
                Some(s) => self.say(s.to_string()),
                None => self.send(&l),
            }
        }
        if let Some((part, local, target)) = self.grab
            && let Some(ps) = core::pull_params(&self.agent.doll, part, local, target)
        {
            let (k, j, hit, equalize) = core::impulse_params(&ps);
            let _ = msg::send(&mut self.agent, &self.build, &self.world, &mut self.msgs, "applyImpulse", &ps);
            self.kicks.extend(core::kick(&self.agent.doll, k, j, hit, equalize));
        }
        // The room's part before the step (the module's `room_before`): the shots, then on even steps its
        // `shot_frame`.
        for at in std::mem::take(&mut self.shoot) {
            let fired = self.shots.fire(&self.agent, at, self.steps);
            self.say(fired.say);
            for l in &fired.lines {
                self.send(l);
            }
        }
        if self.steps % 2 == 0 {
            let (said, lines) = self.shots.frame(&self.agent, self.steps);
            for s in said {
                self.say(s);
            }
            for l in &lines {
                self.send(l);
            }
        }
        let unit = core::balancer_unit(&self.agent, &self.build, dt);
        self.agent.step_follow(&self.build, &self.world, dt, &[], &[]);
        self.steps += 1;
        self.kicks.extend(core::balancer_kicks(&self.agent, unit));
        // NM's feedback, said as it changes (a balancing body repeats its slots every step).
        let feedback: Vec<String> = self.agent.feedback.iter().map(|f| format!("FB {} slot {}", f.name, f.slot)).collect();
        for line in feedback.iter().filter(|l| !self.feedback.contains(l)).cloned().collect::<Vec<_>>() {
            self.say(line);
        }
        self.feedback = feedback;
        core::outputs(&self.agent.doll, &mut self.drives, &mut self.armature)
    }

    /// A physics step of `timestep` s (the module's `begin`, `compute` and `finish`, on this thread).
    ///
    /// # Safety
    /// Inside the environment's step callback; the bodies live.
    unsafe fn physics_step(&mut self, nm: &Nm, timestep: f32) -> Result<(), String> {
        self.due = 0;
        self.acc += f64::from(timestep);
        while self.acc + 1e-9 >= core::DT {
            self.acc -= core::DT;
            if self.due == MAX_STEPS {
                self.acc = 0.0;
                break;
            }
            self.due += 1;
        }
        if self.due > 0 {
            self.snap.clear();
            for &o in &self.objects {
                // SAFETY: the caller's.
                self.snap.push(unsafe { nm.body_state(o) }.ok_or("GetBodyState failed")?);
            }
            let started = std::time::Instant::now();
            for _ in 0..self.due {
                self.nm_step()?;
            }
            let ms = started.elapsed().as_secs_f64() * 1000.0 / f64::from(self.due);
            self.cost += (ms - self.cost) * 0.05;
        }
        for k in 1..PARTS {
            if self.armature[k] != self.armature_set[k] {
                self.armature_set[k] = self.armature[k];
                // SAFETY: a live joint of the body.
                unsafe { nm.set_joint_armature(self.joints[k], self.armature[k]) };
            }
        }
        if self.asleep() && self.kicks.is_empty() {
            return Ok(());
        }
        let forces = core::forces(&self.agent.doll, &self.drives, self.asleep(), self.comp, self.g_src, 0, self.kicks.drain(..), timestep);
        for (&o, (f, t)) in self.objects.iter().zip(&forces) {
            // SAFETY: a live body; the vectors are finite (checked as they came).
            unsafe { nm.apply_body_drive(o, Some(&f.to_array()), Some(&t.to_array())) };
        }
        Ok(())
    }

    /// NM off the body: no armature (the drives stop with it).
    ///
    /// # Safety
    /// The joints are live.
    unsafe fn release(&self, nm: &Nm) {
        for &c in self.joints.iter().skip(1) {
            // SAFETY: the caller's.
            unsafe { nm.set_joint_armature(c, 0.0) };
        }
    }
}

/// The agent the step callback drives.
#[derive(Default)]
struct Driven {
    agent: Option<NmAgent>,
}

/// `PhysicsNMStepFn_t` on the editor's environment: the agent's physics step; an error or a panic takes NM off
/// (the module's `on_step`), the body an ordinary one again.
unsafe extern "C" fn on_step(_env: *mut IPhysicsEnvironment, timestep: f32, context: *mut c_void) {
    // SAFETY: `context` is the `Sim`'s boxed cell, alive while the callback is set (removed on drop).
    let cell = unsafe { &*(context as *const RefCell<Driven>) };
    let (Ok(mut d), Some(nm)) = (cell.try_borrow_mut(), nm::get()) else { return };
    let Some(a) = d.agent.as_mut() else { return };
    // SAFETY: the bodies live while the agent does (it goes before them, `Sim::clear`).
    let r = panic::catch_unwind(AssertUnwindSafe(|| unsafe { a.physics_step(&nm, timestep) }));
    let e = match r {
        Ok(Ok(())) => return,
        Ok(Err(e)) => e,
        Err(p) => format!("panic: {}", p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "?".into())),
    };
    // SAFETY: as above.
    unsafe { a.release(&nm) };
    a.failed = Some(e);
}

/// The fork loaded, its environment and the body in it.
pub struct Sim {
    pub host: &'static Host,
    env: Environment<'static>,
    pub dll: PathBuf,
    pub surfaces: i32,
    body: Option<(NativeBody, Body)>,
    /// NM's agent on the body, the step callback's context (boxed: its address is the callback's).
    driven: Box<RefCell<Driven>>,
    floor_material: u8,
    acc: f32,
    pub time: f32,
    pub steps: u64,
    pub paused: bool,
    pub speed: f32,
    pub grab: Option<Grab>,
    pub nm_gravity: bool,
    /// The step's cost (ms), for the status line.
    pub step_ms: f32,
}

impl Sim {
    /// Loads the fork (`dll`, its imports from `deps`) and makes the environment: the game's surface properties
    /// (`surfaces`: the scripts' names and texts), a floor at z = 0 of `floor_material`, NM's friction and elasticity
    /// on it (`concrete`, the Playroom's floor row; `nm_floor` its row in NM's surfaces, for NM's own floor).
    pub fn new(dll: &Path, deps: &Path, surfaces: &[(String, String)], floor_material: &str, concrete: [f32; 2], nm_floor: u8) -> Result<Sim, String> {
        // One host a process: the DLL can't be loaded twice anyway.
        static HOST: std::sync::OnceLock<Result<&'static Host, String>> = std::sync::OnceLock::new();
        let host = HOST.get_or_init(|| Host::load(dll, deps).map(|h| &*Box::leak(Box::new(h)))).clone()?;
        let mut count = 0;
        for (name, text) in surfaces {
            count = host.parse_surfaces(name, text);
        }
        let mut env = host.environment([0.0, 0.0, -SOURCE_GRAVITY], TIMESTEP)?;
        let material = host.surface_index(floor_material);
        if material >= 0 {
            host.nm.set_surface_material(material, concrete[0], concrete[1]);
        }
        env.static_box([-4000.0, -4000.0, -64.0], [4000.0, 4000.0, 0.0], material)?;
        let driven = Box::new(RefCell::new(Driven::default()));
        // SAFETY: our live environment; the context outlives the callback (removed on drop, before the box goes).
        if !unsafe { host.nm.set_step_callback(env.env, Some(on_step), &*driven as *const RefCell<Driven> as *mut c_void) } {
            return Err("SetStepCallback failed".into());
        }
        let mut s = Sim {
            host,
            env,
            dll: dll.to_path_buf(),
            surfaces: count,
            body: None,
            driven,
            floor_material: nm_floor,
            acc: 0.0,
            time: 0.0,
            steps: 0,
            paused: false,
            speed: 1.0,
            grab: None,
            nm_gravity: true,
            step_ms: 0.0,
        };
        s.set_gravity(true);
        Ok(s)
    }

    /// The gravity: NM's 9.8 m/s^2 (what the body feels in the game under NM: Source's and the module's
    /// compensating force, docs/adr/0007) or Source's own 600 units/s^2. With NM on the body, the environment keeps
    /// Source's and the agent makes up the difference as the module's do; without, the environment has NM's.
    pub fn set_gravity(&mut self, nm: bool) {
        self.nm_gravity = nm;
        let on = {
            let mut d = self.driven.borrow_mut();
            if let Some(a) = d.agent.as_mut() {
                a.comp = if nm { 1.0 } else { 0.0 };
            }
            d.agent.is_some()
        };
        let g = if nm && !on { nm_gravity() / M } else { SOURCE_GRAVITY };
        // SAFETY: our live environment, outside its step.
        unsafe { nm::env::set_gravity(self.env.env, [0.0, 0.0, -g]) };
    }

    pub fn has_body(&self) -> bool {
        self.body.is_some()
    }

    /// The body in it, as NM's parts.
    pub fn body(&self) -> Option<&Body> {
        self.body.as_ref().map(|b| &b.1)
    }

    /// The body taken out (NM first).
    pub fn clear(&mut self) {
        self.nm_off();
        if let Some((b, _)) = self.body.take() {
            // SAFETY: made by `place`, on this thread.
            unsafe { b.destroy(&self.host.nm) };
        }
        self.grab = None;
    }

    /// `body` made anew standing as NM puts an agent (feet at `lift` m above the floor), turned by `tilt` (rad about
    /// X) and moving at `vel` (m/s), spinning at `spin` (rad/s).
    pub fn place(&mut self, body: &Body, lift: f32, tilt: f32, vel: Vec3, spin: Vec3) -> Result<(), String> {
        self.clear();
        let placed = body.placed(Vec3::new(0.0, 0.0, lift), 0.0);
        let centre = placed.iter().map(|p| p.0).sum::<Vec3>() / placed.len() as f32;
        let turn = Quat::from_rotation_x(tilt);
        let states: Vec<PartState> = placed
            .into_iter()
            .map(|(p, q)| {
                let pos = centre + turn * (p - centre);
                PartState { pos, rot: (turn * q).normalize(), vel: vel + spin.cross(pos - centre), spin }
            })
            .collect();
        // SAFETY: our live environment, on its thread, outside its step.
        let made = unsafe { NativeBody::make(&self.host.nm, self.env.env, std::ptr::null_mut(), body, &states, 0)? };
        self.body = Some((made, body.clone()));
        self.time = 0.0;
        self.steps = 0;
        Ok(())
    }

    /// NM (the Playroom's agent, `build` the body's, `data` what its shot reads) on the body as it is, its button
    /// `button` pressed (`balance`: `bodyBalance start=true`).
    pub fn nm_on(&mut self, build: &Build, data: &Arc<GameData>, button: &str) -> Result<(), String> {
        let lines = core::button_lines(button).ok_or_else(|| format!("no Playroom button {button}"))?;
        self.nm_off();
        let Some((made, body)) = &self.body else { return Err("no body to put NM on".into()) };
        if build.parts.len() != PARTS || body.parts.len() != PARTS {
            return Err(format!("NM's body has {PARTS} parts"));
        }
        // SAFETY: the body is live; the agent goes before it (`clear`).
        let mut a = unsafe { NmAgent::new(&self.host.nm, made, build, data.clone(), self.floor_material, 0.0, self.nm_gravity)? };
        for l in lines {
            a.queue(l);
        }
        a.button = button.to_string();
        self.driven.borrow_mut().agent = Some(a);
        self.set_gravity(self.nm_gravity);
        Ok(())
    }

    /// NM off the body: an ordinary ragdoll again.
    pub fn nm_off(&mut self) {
        let Some(a) = self.driven.borrow_mut().agent.take() else { return };
        // SAFETY: the body is still in (`clear` takes NM off first).
        unsafe { a.release(&self.host.nm) };
        self.set_gravity(self.nm_gravity);
    }

    /// Runs `f` on NM's agent, if it's on.
    pub fn with_nm<R>(&self, f: impl FnOnce(&mut NmAgent) -> R) -> Option<R> {
        self.driven.borrow_mut().agent.as_mut().map(f)
    }

    /// The parts' states now.
    pub fn states(&self) -> Option<Vec<PartState>> {
        // SAFETY: a live body of ours.
        self.body.as_ref().and_then(|(b, _)| unsafe { b.states(&self.host.nm) }.ok())
    }

    /// Every part given `vel` more (m/s) and `spin` (rad/s about the body's centre).
    pub fn kick(&mut self, vel: Vec3, spin: Vec3) {
        let Some(states) = self.states() else { return };
        let Some((b, _)) = &self.body else { return };
        let centre = states.iter().map(|s| s.pos).sum::<Vec3>() / states.len() as f32;
        for (s, &o) in states.iter().zip(&b.objects) {
            let v = s.vel + vel + spin.cross(s.pos - centre);
            let state = PhysicsNMBodyState { position: s.pos.to_array(), rotation: s.rot.to_array(), velocity: v.to_array(), angular_velocity: (s.spin + spin).to_array() };
            // SAFETY: a live body of ours, outside the step.
            unsafe { self.host.nm.set_body_state(o, &state) };
        }
    }

    /// The Playroom's click (`poke`) under NM: `impulse` N s into part `k` at its centre along `dir`, as its line.
    pub fn poke(&mut self, k: usize, dir: Vec3, impulse: f32) {
        let Some(at) = self.states().and_then(|s| s.get(k).map(|p| p.pos)) else { return };
        self.with_nm(|a| a.queue(&core::poke_line(k, at, dir, impulse)));
    }

    /// The room's shot under NM (its `shot` and `shot back` buttons, the weapon picked as by `!weapon`).
    pub fn shoot(&mut self, at: ShotAt) {
        self.with_nm(|a| a.shoot(at));
    }

    /// A message line (`<message> [param=value ...]`) or the Playroom's button (`balance`, `relax`, `STOP all`, ...)
    /// to NM for its next step.
    pub fn send(&mut self, line: &str) {
        let lines: Vec<String> = match core::button_lines(line.trim()) {
            Some(ls) => ls.iter().map(|l| l.to_string()).collect(),
            None => vec![line.to_string()],
        };
        self.with_nm(|a| lines.iter().for_each(|l| a.queue(l)));
    }

    /// The physics run on by `dt` of real time (s), in the game's steps; the mouse's pull applied each step: as the
    /// physgun pulls (an impulse toward the target, damped), or under NM the Playroom's grab (`pull`).
    pub fn run(&mut self, dt: f32) {
        if self.paused {
            return;
        }
        self.acc = (self.acc + dt.min(0.1) * self.speed).min(TIMESTEP * 8.0);
        let started = std::time::Instant::now();
        let mut stepped = 0;
        while self.acc >= TIMESTEP {
            self.acc -= TIMESTEP;
            self.step();
            stepped += 1;
        }
        if stepped > 0 {
            self.step_ms = started.elapsed().as_secs_f32() * 1000.0 / stepped as f32;
        }
    }

    /// One physics step.
    pub fn step(&mut self) {
        let grab = self.grab.as_ref().map(|g| (g.part, g.local, g.target));
        if self.with_nm(|a| a.grab = grab).is_none() {
            self.pull();
        }
        self.env.simulate(TIMESTEP);
        self.time += TIMESTEP;
        self.steps += 1;
    }

    fn pull(&mut self) {
        let (Some(g), Some((b, _))) = (&self.grab, &self.body) else { return };
        let o = b.objects[g.part];
        // SAFETY: a live body of ours, outside the step.
        let Some(s) = (unsafe { self.host.nm.body_state(o) }) else { return };
        let (pos, rot) = (Vec3::from_array(s.position), Quat::from_array(s.rotation).normalize());
        let at = pos + rot * g.local;
        let v = Vec3::from_array(s.velocity) + Vec3::from_array(s.angular_velocity).cross(at - pos);
        // Toward the target in about a tenth of a second, at most 10 m/s, damped; on the part's own mass.
        let want = ((g.target - at) / 0.1).clamp_length_max(10.0);
        // SAFETY: as above.
        let mass = unsafe { nm::object::mass(o) };
        let impulse = (want - v) * mass * 0.5 / M;
        unsafe {
            nm::object::apply_force_offset(o, impulse.to_array(), (at / M).to_array());
            nm::object::wake(o);
        }
    }
}

impl Drop for Sim {
    fn drop(&mut self) {
        self.clear();
        // SAFETY: our live environment; its context goes after this.
        unsafe { self.host.nm.set_step_callback(self.env.env, None, std::ptr::null_mut()) };
    }
}
