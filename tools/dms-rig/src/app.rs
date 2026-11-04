//! The editor's window: the models on the left, the 3D view in the middle (the model in its own pose, NM's parts over
//! it, the handles that edit them), the rig's numbers and file on the right; the simulation on the fork in its tab.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use delirium::prelude::*;
use dms_fit::M;
use dms_fit::fit::Fit;
use dms_fit::rigs::{self, PARTS, SPREAD};
use dms_fit::skeleton::{Skeleton, read_mdl};
use dms_game::body::Shape;
use eframe::egui::{self, Color32, Pos2, Stroke};

use crate::gamefs::GameFs;
use crate::mesh::{self, Mesh};
use crate::render::{Batch, Camera, Renderer, Scene, Shared, View};
use crate::rigfile::{self, Rig};
use crate::scene::{self, Hulls};
use crate::sim::{self, Sim};
use crate::snap::{self, placement, shape_rot};
use crate::{Homer, Options};

/// A model open in the editor.
struct Model {
    path: String,
    source: String,
    skel: Skeleton,
    mesh: Option<Arc<Mesh>>,
    batch: Option<Arc<Batch>>,
    version: u64,
    problems: Vec<String>,
    /// The model's own rig (no file): its class and bones.
    auto_rig: String,
    auto_map: [Option<String>; 21],
    rig: Rig,
    /// The file as it was read or last saved (`None`: none).
    saved: Option<Rig>,
    fit: Result<Fit, String>,
    undo: Vec<Rig>,
    redo: Vec<Rig>,
}

impl Model {
    /// The bone at each part now: the rig's mapping, else the model's own.
    fn map(&self) -> [Option<String>; 21] {
        if self.rig.bones.is_empty() {
            return self.auto_map.clone();
        }
        std::array::from_fn(|k| self.rig.bones.iter().find(|(p, _)| p.eq_ignore_ascii_case(PARTS[k])).map(|(_, b)| b.clone()))
    }

    fn file(&self, gmod: &std::path::Path) -> PathBuf {
        gmod.join("garrysmod").join("data").join("dmr").join("skeletons").join(rigfile::file_name(&self.path))
    }
}

fn refit(homer: &Homer, m: &mut Model) {
    let (names, edits) = m.rig.args();
    m.fit = dms_fit::fit::fit(&homer.build, &homer.body, &m.skel, &m.path, names, edits);
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Handle {
    /// A joint's dot: dragged on the screen's plane.
    Joint(usize),
    Radius(usize),
    Half(usize, f32),
    BoxAxis(usize, usize, f32),
    /// The move gizmo's centre: on the screen's plane.
    Free(usize),
    /// A move arrow.
    Axis(usize, usize),
    /// A turn ring; 3 turns about the view's axis.
    Ring(usize, usize),
    Bone(usize),
    Part(usize),
}

/// The gizmo the selected part shows.
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Move,
    Rotate,
    Scale,
    All,
}

impl Tool {
    const ALL: [(Tool, &'static str, egui::Key, &'static str); 5] = [
        (Tool::Select, "Select", egui::Key::Q, "Q: select only; joints still drag"),
        (Tool::Move, "Move", egui::Key::W, "W: arrows along the axes, the centre on the screen's plane"),
        (Tool::Rotate, "Rotate", egui::Key::E, "E: rings about the axes, the outer one about the view"),
        (Tool::Scale, "Scale", egui::Key::R, "R: the capsule's radius and length, the box's extents"),
        (Tool::All, "All", egui::Key::T, "T: move, rotate and scale at once"),
    ];

    fn moves(self) -> bool {
        matches!(self, Tool::Move | Tool::All)
    }

    fn turns(self) -> bool {
        matches!(self, Tool::Rotate | Tool::All)
    }

    fn scales(self) -> bool {
        matches!(self, Tool::Scale | Tool::All)
    }
}

/// The gizmo's axes.
#[derive(Clone, Copy, PartialEq)]
enum Space {
    /// The part's own: a capsule's Y is its length, along its bone.
    Part,
    /// The model's bone mapped to the part (the part's own for the spread spine).
    Bone,
    World,
}

/// The selected part's gizmo: where its handles stand and along what.
struct Gizmo {
    k: usize,
    /// The move handles move the joint (else the part on its bone).
    joint: bool,
    /// The move handles' centre.
    at: Vec3,
    /// The rings' centre: the part's, or its joint's.
    pivot: Vec3,
    axes: [Vec3; 3],
    /// The arrows' length and the rings' radius (m), the same on the screen at any distance.
    arrow: f32,
    ring: f32,
}

const ARROW_PX: f32 = 95.0;
const RING_PX: f32 = 70.0;
const AXIS_COLORS: [Color32; 3] = [Color32::from_rgb(235, 70, 70), Color32::from_rgb(80, 210, 80), Color32::from_rgb(80, 120, 255)];
const HOT: Color32 = Color32::from_rgb(255, 230, 60);

/// A drag in progress: what, from where, and the values it started from.
struct Drag {
    handle: Handle,
    press: Pos2,
    rig0: Rig,
    shown0: Transform,
    dir: Vec3,
    point0: Vec3,
    start: Vec3,
    /// The move acts on the joint.
    joint: bool,
    /// The joints as the drag started (m), for IK.
    joints0: Vec<Vec3>,
    ik: bool,
}

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Rig,
    Sim,
}

pub struct App {
    homer: Homer,
    fs: GameFs,
    opts: Options,
    models: Vec<(String, usize)>,
    filter: String,
    workshop_only: bool,
    classes: HashMap<String, String>,
    cur: Option<Model>,
    status: String,
    renderer: Shared,
    cam: Camera,
    hulls: Hulls,
    sel: Option<usize>,
    pick: Option<usize>,
    drag: Option<Drag>,
    hover: Option<Handle>,
    mesh_alpha: f32,
    parts_alpha: f32,
    show_bones: bool,
    tab: Tab,
    sim: Option<Sim>,
    sim_err: Option<String>,
    /// Homer's own body simulated (golden's), not the model's fit.
    sim_homer: bool,
    /// The push under NM (N s, the Playroom's poke by default).
    push: f32,
    /// The room's shot under NM: the part, the weapon, from behind.
    shot_at: sim::ShotAt,
    /// The message line being typed, and the lines sent (the newest last).
    nm_line: String,
    nm_sent: Vec<String>,
    paste: String,
    import: String,
    shot: Option<(PathBuf, u64)>,
    shot_sent: bool,
    frames: u64,
    last: std::time::Instant,
    version: u64,
    /// A number being dragged in the panel has pushed its undo step.
    number_drag: bool,
    tool: Tool,
    space: Space,
    /// The rings turn the part about its joint (else its centre).
    pivot_joint: bool,
    /// The joint was clicked last: the move gizmo moves it.
    target_joint: bool,
    /// An edit of a limb's part or joint is mirrored on the other side.
    symmetry: bool,
    /// A hand's or foot's joint dragged carries its elbow or knee (the limb's lengths kept); an elbow or knee turns
    /// about its limb's line.
    ik: bool,
}

const HANDLE_PX: f32 = 7.0;

/// The limbs' parts, left and right.
const PAIRS: [(usize, usize); 7] = [(1, 4), (2, 5), (3, 6), (13, 17), (14, 18), (15, 19), (16, 20)];
/// The limbs' joints: the hip or shoulder, the knee or elbow, the ankle or wrist.
const LIMBS: [[usize; 3]; 4] = [[1, 2, 3], [4, 5, 6], [14, 15, 16], [18, 19, 20]];

/// The model's plane of symmetry: through the middle of its hips, across their line (kept upright).
fn mirror_plane(m: &Model) -> Option<(Vec3, Vec3)> {
    let map = m.map();
    let at = |k: usize| map[k].as_ref().and_then(|n| m.skel.bones.iter().find(|b| &b.name == n)).map(|b| b.bind.0 * M);
    let (l, r) = (at(1)?, at(4)?);
    let n = Vec3::new(l.x - r.x, l.y - r.y, 0.0).normalize_or_zero();
    (n != Vec3::ZERO).then_some(((l + r) * 0.5, n))
}

/// The limbs' edits changed since `before` copied to their other sides, mirrored across the model's plane: joints'
/// moves, sizes and masses as they are, the parts' placement as the other side's mirror image.
fn mirror(homer: &Homer, m: &mut Model, before: &Rig) {
    let Some((o, n)) = mirror_plane(m) else { return };
    let flip = |v: Vec3| v - 2.0 * n * n.dot(v);
    let mut placed = vec![];
    let mut any = false;
    for (a, b) in PAIRS {
        let changed = |k: usize| m.rig.edits.parts[k] != before.edits.parts[k] || m.rig.edits.joints[k] != before.edits.joints[k];
        let (from, to) = if changed(a) { (a, b) } else if changed(b) { (b, a) } else { continue };
        any = true;
        let (e0, e1) = (before.edits.parts[from], m.rig.edits.parts[from]);
        if m.rig.edits.joints[from] != before.edits.joints[from] {
            m.rig.edits.joints[to] = flip(m.rig.edits.joints[from]);
        }
        let t = &mut m.rig.edits.parts[to];
        if e1.radius != e0.radius {
            t.radius = e1.radius;
        }
        if e1.half != e0.half {
            t.half = e1.half;
        }
        if e1.boxed != e0.boxed {
            t.boxed = e1.boxed;
        }
        if e1.mass != e0.mass {
            t.mass = e1.mass;
        }
        if e1.offset != e0.offset || e1.turn != e0.turn {
            placed.push((from, to));
        }
    }
    if !any {
        return;
    }
    refit(homer, m);
    if placed.is_empty() {
        return;
    }
    let Ok(f) = &m.fit else { return };
    let s = Mat3::IDENTITY - 2.0 * Mat3::from_cols(n * n.x, n * n.y, n * n.z);
    let set: Vec<(usize, Vec3, Vec3)> = placed
        .into_iter()
        .map(|(from, to)| {
            // The mirror image's frame: reflected, then its own X flipped back (a capsule's and a box's shape keep).
            let src = Mat3::from_quat(f.shown[from].rotation * shape_rot(&f.body.parts[from].shape));
            let want = Quat::from_mat3(&(s * src * Mat3::from_diagonal(Vec3::new(-1.0, 1.0, 1.0)))).normalize();
            let at = o + flip(f.shown[from].translation - o);
            let (offset, turn) = placement(f, to, m.rig.edits.parts[to], at, want);
            (to, offset, turn)
        })
        .collect();
    for (to, offset, turn) in set {
        m.rig.edits.parts[to].offset = offset;
        m.rig.edits.parts[to].turn = turn;
    }
    refit(homer, m);
}

/// Two-bone IK: the limb's joints with its end at `want`, its root kept, its lengths kept, bent the way it was.
fn reach(root: Vec3, mid: Vec3, end: Vec3, want: Vec3) -> (Vec3, Vec3) {
    let (l1, l2) = (mid.distance(root), end.distance(mid));
    let d = want - root;
    let dist = d.length().clamp((l1 - l2).abs() + 1e-4, l1 + l2 - 1e-4);
    let u = d.normalize_or(Vec3::Z);
    let pole = mid - root;
    let b = (pole - u * pole.dot(u)).normalize_or(u.any_orthonormal_vector());
    let c = ((l1 * l1 + dist * dist - l2 * l2) / (2.0 * l1 * dist)).clamp(-1.0, 1.0);
    (root + u * (l1 * c) + b * (l1 * (1.0 - c * c).sqrt()), root + u * dist)
}

/// The middle joint turned about the limb's line toward `want`, its lengths kept.
fn swing(root: Vec3, mid: Vec3, end: Vec3, want: Vec3) -> Vec3 {
    let e = (end - root).normalize_or(Vec3::Z);
    let c = root + e * (mid - root).dot(e);
    let r = (mid - c).length();
    let w = want - c;
    let dir = (w - e * w.dot(e)).normalize_or(mid - c);
    c + dir.normalize_or(Vec3::X) * r
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>, homer: Homer, fs: GameFs, opts: Options) -> App {
        let models = fs.models();
        let shot = opts.screenshot.clone();
        let sim_homer = opts.homer;
        let mut app = App {
            homer,
            models,
            opts,
            filter: String::new(),
            workshop_only: false,
            classes: HashMap::new(),
            cur: None,
            status: String::new(),
            renderer: Arc::new(Mutex::new(Renderer::default())),
            cam: Camera::default(),
            hulls: Hulls::default(),
            sel: None,
            pick: None,
            drag: None,
            hover: None,
            mesh_alpha: 0.45,
            parts_alpha: 1.0,
            show_bones: true,
            tab: Tab::Rig,
            sim: None,
            sim_err: None,
            sim_homer,
            push: dms_vphysics::follow::POKE,
            shot_at: sim::ShotAt::default(),
            nm_line: String::new(),
            nm_sent: vec![],
            paste: String::new(),
            import: String::new(),
            shot,
            shot_sent: false,
            frames: 0,
            last: std::time::Instant::now(),
            version: 0,
            number_drag: false,
            tool: Tool::All,
            space: Space::Part,
            pivot_joint: false,
            target_joint: false,
            symmetry: false,
            ik: false,
            fs,
        };
        app.status = format!("{} models; {} sources{}", app.models.len(), app.fs.sources.len(),
            if app.fs.problems.is_empty() { String::new() } else { format!("; {} unreadable (first: {})", app.fs.problems.len(), app.fs.problems[0]) });
        if let Some(t) = &app.opts.tool {
            app.tool = Tool::ALL.iter().find(|x| x.1.eq_ignore_ascii_case(t)).map_or(app.tool, |x| x.0);
        }
        if let Some(m) = app.opts.model.clone() {
            app.open(&m);
            if let Some(y) = app.opts.yaw {
                app.cam.yaw = y.to_radians();
            }
            app.sel = app.opts.select.as_ref().and_then(|s| PARTS.iter().position(|p| p.eq_ignore_ascii_case(s)));
            if app.opts.snap {
                app.snap((0..21).collect());
                println!("dms-rig: {}", app.status);
            }
            // A screenshot of a part: framed close.
            if let (Some(k), Some(_), Some(Ok(f))) = (app.sel, &app.shot, app.cur.as_ref().map(|m| &m.fit)) {
                app.cam.target = f.shown[k].translation;
                app.cam.distance = 0.6;
            }
        }
        if app.shot.as_ref().is_some_and(|s| s.1 > 0) {
            app.tab = Tab::Sim;
            app.start_sim();
            match app.opts.nm.clone() {
                Some(b) => app.sim_nm(&b),
                None => app.sim_drop(),
            }
        }
        app
    }

    fn open(&mut self, path: &str) {
        match self.load(path) {
            Ok(m) => {
                let note = m.fit.as_ref().err().cloned();
                // Framed: the mesh's bounds, or the bones'.
                let pts: Vec<Vec3> = match &m.mesh {
                    Some(me) => me.pos.iter().map(|p| Vec3::from_array(*p) * M).collect(),
                    None => m.skel.bones.iter().map(|b| b.pose.0 * M).collect(),
                };
                let (lo, hi) = pts.iter().fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(a, b), p| (a.min(*p), b.max(*p)));
                if lo.x <= hi.x {
                    self.cam.target = (lo + hi) * 0.5;
                    self.cam.distance = (hi - lo).length().clamp(0.3, 20.0) * 1.3;
                }
                self.status = format!("{path}: {} bones, {}{}", m.skel.bones.len(), m.auto_rig, note.map(|n| format!(" — {n}")).unwrap_or_default());
                self.cur = Some(m);
                self.sel = None;
                self.pick = None;
                if let Some(s) = &mut self.sim {
                    s.clear();
                }
            }
            Err(e) => self.status = format!("{path}: {e}"),
        }
    }

    fn load(&mut self, path: &str) -> Result<Model, String> {
        let bytes = self.fs.read(path)?;
        let skel = read_mdl(&bytes)?;
        let base = path.strip_suffix(".mdl").unwrap_or(path);
        let mut problems = vec![];
        let mesh = match (self.fs.read(&format!("{base}.vvd")), self.fs.read(&format!("{base}.dx90.vtx"))) {
            (Ok(vvd), Ok(vtx)) => mesh::read(&bytes, &vvd, &vtx).map_err(|e| problems.push(format!("mesh: {e}"))).ok(),
            (Err(e), _) | (_, Err(e)) => {
                problems.push(format!("mesh: {e}"));
                None
            }
        };
        let batch = mesh.as_ref().map(|m| Arc::new(scene::mesh_batch(m, [0.78, 0.76, 0.72, 1.0])));
        self.version += 1;
        let at: Vec<Vec3> = skel.bones.iter().map(|b| b.bind.0).collect();
        let (auto_rig, auto_map) = match rigs::classify(&skel, None, &at) {
            Ok(c) => (if c.complete() { c.rig.clone() } else { format!("{} ({})", c.rig, c.why(path)) }, std::array::from_fn(|k| c.map[k].map(|b| skel.bones[b].name.clone()))),
            Err(e) => (e, std::array::from_fn(|_| None)),
        };
        let source = self.models.iter().find(|m| m.0 == path).map(|m| self.fs.sources[m.1].name.clone()).unwrap_or_default();
        let mut m = Model {
            path: path.to_string(),
            source,
            skel,
            mesh: mesh.map(Arc::new),
            batch,
            version: self.version,
            problems,
            auto_rig,
            auto_map,
            rig: Rig::default(),
            saved: None,
            fit: Err(String::new()),
            undo: vec![],
            redo: vec![],
        };
        let file = m.file(&self.opts.gmod);
        if let Ok(text) = std::fs::read_to_string(&file) {
            match rigfile::decode(&text) {
                Ok(r) => {
                    m.rig = r.clone();
                    m.saved = Some(r);
                }
                Err(e) => m.problems.push(format!("{}: {e}", file.display())),
            }
        }
        m.rig.model = Some(m.path.clone());
        m.rig.checksum = Some(m.skel.checksum);
        refit(&self.homer, &mut m);
        Ok(m)
    }

    /// An edit of the rig, undoable.
    fn edit(&mut self, f: impl FnOnce(&mut Rig)) {
        let Some(m) = &mut self.cur else { return };
        let before = m.rig.clone();
        f(&mut m.rig);
        if m.rig != before {
            m.undo.push(before);
            m.redo.clear();
            refit(&self.homer, m);
        }
    }

    fn undo(&mut self, redo: bool) {
        let Some(m) = &mut self.cur else { return };
        let (from, to) = if redo { (&mut m.redo, &mut m.undo) } else { (&mut m.undo, &mut m.redo) };
        if let Some(r) = from.pop() {
            to.push(std::mem::replace(&mut m.rig, r));
            refit(&self.homer, m);
        }
    }

    /// The parts laid on the model's bones and mesh by our own placement (`snap`): their sizes, then where they lie on
    /// their bones; one undo step, mirrored when symmetric.
    fn snap(&mut self, parts: Vec<usize>) {
        let Some(m) = &mut self.cur else { return };
        let Some(mesh) = &m.mesh else {
            self.status = "snap: needs the model's mesh".into();
            return;
        };
        let before = m.rig.clone();
        let phy = self.fs.read(&format!("{}.phy", m.path.strip_suffix(".mdl").unwrap_or(&m.path))).ok();
        let done = match snap::snap_rig(&self.homer, &m.skel, mesh, phy.as_deref(), &m.path, &m.rig, &m.map(), &parts) {
            Ok(d) => d,
            Err(e) => {
                self.status = format!("snap: {e}");
                return;
            }
        };
        m.rig = done.rig;
        refit(&self.homer, m);
        if self.symmetry && parts.len() == 1 {
            mirror(&self.homer, m, &before);
        }
        if m.rig != before {
            m.undo.push(before);
            m.redo.clear();
        }
        self.status = done.status;
    }

    /// Part `k`'s bone set (`None`: none); the mapping kept only where it differs from the model's own.
    fn set_bone(&mut self, k: usize, bone: Option<String>) {
        let Some(m) = &self.cur else { return };
        let mut map = m.map();
        map[k] = bone;
        let auto = m.auto_map.clone();
        self.edit(|r| {
            r.bones = if map == auto { vec![] } else { (0..21).filter(|k| !SPREAD.contains(k)).filter_map(|k| map[k].clone().map(|b| (PARTS[k].to_string(), b))).collect() };
        });
    }

    fn save(&mut self) {
        let Some(m) = &mut self.cur else { return };
        let file = m.file(&self.opts.gmod);
        let r = file.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|_| std::fs::write(&file, rigfile::encode(&m.rig)));
        self.status = match r {
            Ok(()) => {
                m.saved = Some(m.rig.clone());
                format!("saved {} (the game reloads it within a second)", file.display())
            }
            Err(e) => format!("{}: {e}", file.display()),
        };
    }

    fn take_rig(&mut self, text: &str, from: &str) {
        match rigfile::decode(text) {
            Ok(mut r) => {
                let note = match (&self.cur, r.checksum) {
                    (Some(m), Some(c)) if c != m.skel.checksum => " (made for another build of the model: its bones are matched by name)",
                    _ => "",
                };
                if let Some(m) = &self.cur {
                    r.model = Some(m.path.clone());
                    r.checksum = Some(m.skel.checksum);
                }
                self.edit(|x| *x = r);
                self.status = format!("rig from {from}{note}");
            }
            Err(e) => self.status = format!("{from}: {e}"),
        }
    }

    fn start_sim(&mut self) {
        if self.sim.is_some() {
            return;
        }
        let dlls = match &self.opts.vphysics {
            Some(p) => vec![p.clone()],
            None => sim::find_vphysics(&self.opts.gmod, self.opts.repo.as_deref()),
        };
        let Some(dll) = dlls.first() else {
            self.sim_err = Some("no vphysics.dll of the fork (scripts\\install.ps1 -Fork, or --vphysics)".into());
            return;
        };
        let deps = self.opts.gmod.join("bin").join("win64");
        match Sim::new(dll, &deps, &self.fs.surface_scripts(), "concrete", self.homer.concrete, self.homer.floor_material) {
            Ok(s) => {
                self.sim = Some(s);
                self.sim_err = None;
            }
            Err(e) => self.sim_err = Some(e),
        }
    }

    /// The body to simulate and its build: the model's fit, or Homer's own.
    fn sim_body(&self) -> Option<(delirium::ragdoll::Build, dms_game::body::Body)> {
        if self.sim_homer {
            return Some((self.homer.build.clone(), self.homer.body.clone()));
        }
        self.cur.as_ref().and_then(|m| m.fit.as_ref().ok()).map(|f| (f.build.clone(), f.body.clone()))
    }

    fn sim_place(&mut self, lift: f32, tilt: f32, vel: Vec3, spin: Vec3) {
        self.start_sim();
        let Some((_, body)) = self.sim_body() else {
            self.status = "no fitted body to simulate".into();
            return;
        };
        if let Some(s) = &mut self.sim
            && let Err(e) = s.place(&body, lift, tilt, vel, spin)
        {
            self.sim_err = Some(e);
        }
    }

    fn sim_drop(&mut self) {
        self.sim_place(1.0, 0.4, Vec3::ZERO, Vec3::ZERO);
    }

    /// The body stood and NM put on it with the Playroom's `button` (`balance`: bodyBalance).
    fn sim_nm(&mut self, button: &str) {
        self.sim_place(0.0, 0.0, Vec3::ZERO, Vec3::ZERO);
        let Some((build, _)) = self.sim_body() else { return };
        let Some(s) = &mut self.sim else { return };
        if !s.has_body() {
            return;
        }
        if let Err(e) = s.nm_on(&build, &self.homer.data, button) {
            self.sim_err = Some(e);
        }
    }

    /// The Playroom's poke under NM: the push into its part 9, away from the view.
    fn sim_push(&mut self) {
        let dir = (self.cam.target - self.cam.eye()).with_z(0.0).normalize_or(Vec3::Y);
        if let Some(s) = &mut self.sim {
            s.poke(dms_vphysics::follow::POKE_PART, dir, self.push);
        }
    }

    // ---- the 3D view -------------------------------------------------------------------------------------------

    /// The selected part's gizmo, in the chosen axes.
    fn gizmo(&self, view: &View) -> Option<Gizmo> {
        let k = self.sel?;
        let m = self.cur.as_ref()?;
        let f = m.fit.as_ref().ok()?;
        let s = f.shown[k];
        let own = s.rotation * shape_rot(&f.body.parts[k].shape);
        let q = match self.space {
            Space::World => Quat::IDENTITY,
            Space::Bone => m.map()[k].as_ref().and_then(|n| m.skel.bones.iter().find(|b| &b.name == n)).map_or(own, |b| b.bind.1),
            Space::Part => own,
        };
        let joint = self.target_joint;
        let at = if joint { f.shown_joints[k] } else { s.translation };
        let pivot = if self.pivot_joint { f.shown_joints[k] } else { s.translation };
        Some(Gizmo { k, joint, at, pivot, axes: Vec3::AXES.map(|a| (q * a).normalize()), arrow: ARROW_PX * view.pixel(at), ring: RING_PX * view.pixel(pivot) })
    }

    /// The handles drawn as points: the joints, the tool's scale handles, the move centre and the arrows' tips.
    fn handles(&self, view: &View) -> Vec<(Handle, Vec3)> {
        let mut out = vec![];
        let Some(m) = &self.cur else { return out };
        let Ok(f) = &m.fit else { return out };
        for (k, j) in f.shown_joints.iter().enumerate() {
            out.push((Handle::Joint(k), *j));
        }
        let Some(g) = self.gizmo(view) else { return out };
        let k = g.k;
        if self.tool.scales() {
            let s = f.shown[k];
            let c = s.translation;
            match f.body.parts[k].shape {
                Shape::Capsule { rot, radius, half } => {
                    let axis = s.rotation * rot * Vec3::Y;
                    let perp = axis.cross(view.eye - c).normalize_or(Vec3::Z);
                    out.push((Handle::Radius(k), c + perp * radius));
                    out.push((Handle::Half(k, 1.0), c + axis * (half + radius)));
                    out.push((Handle::Half(k, -1.0), c - axis * (half + radius)));
                }
                Shape::Box { rot, half } => {
                    for i in 0..3 {
                        for sign in [1.0, -1.0] {
                            out.push((Handle::BoxAxis(k, i, sign), c + s.rotation * rot * (Vec3::AXES[i] * half[i] * sign)));
                        }
                    }
                }
            }
        }
        if self.tool.moves() {
            out.push((Handle::Free(k), g.at));
            for i in 0..3 {
                out.push((Handle::Axis(k, i), g.at + g.axes[i] * g.arrow));
            }
        }
        out
    }

    /// Ring `i` of the gizmo on the screen (3: about the view's axis, a little wider).
    fn ring(view: &View, g: &Gizmo, i: usize) -> Vec<Pos2> {
        let (axis, r) = if i < 3 { (g.axes[i], g.ring) } else { ((view.eye - g.pivot).normalize_or(Vec3::Z), g.ring * 1.2) };
        let u = axis.any_orthonormal_vector();
        let v = axis.cross(u);
        (0..=64)
            .filter_map(|n| {
                let a = n as f32 / 64.0 * std::f32::consts::TAU;
                view.project(g.pivot + (u * a.cos() + v * a.sin()) * r)
            })
            .collect()
    }

    /// What's under the mouse in the rig tab.
    fn hit(&self, view: &View, at: Pos2) -> Option<Handle> {
        let m = self.cur.as_ref()?;
        let near = |p: Vec3, px: f32| view.project(p).is_some_and(|s| s.distance(at) <= px);
        // The bone to pick first when picking.
        if self.pick.is_some() {
            return (0..m.skel.bones.len())
                .filter_map(|b| view.project(m.skel.bones[b].pose.0 * M).map(|s| (b, s.distance(at))))
                .filter(|x| x.1 <= 10.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|x| Handle::Bone(x.0));
        }
        let hs = self.handles(view);
        // The gizmo over the joints: its points, its arrows' shafts, its rings (the nearest).
        if let Some(h) = hs.iter().filter(|h| !matches!(h.0, Handle::Joint(_))).find(|h| near(h.1, HANDLE_PX + 2.0)) {
            return Some(h.0);
        }
        if let Some(g) = self.gizmo(view) {
            if self.tool.moves()
                && let Some(c) = view.project(g.at)
            {
                for i in 0..3 {
                    if let Some(t) = view.project(g.at + g.axes[i] * g.arrow)
                        && seg_dist(at, c, t) < 5.0
                    {
                        return Some(Handle::Axis(g.k, i));
                    }
                }
            }
            if self.tool.turns() {
                let best = (0..4)
                    .filter_map(|i| Self::ring(view, &g, i).windows(2).map(|w| seg_dist(at, w[0], w[1])).min_by(f32::total_cmp).map(|d| (i, d)))
                    .filter(|x| x.1 < 5.0)
                    .min_by(|a, b| a.1.total_cmp(&b.1));
                if let Some((i, _)) = best {
                    return Some(Handle::Ring(g.k, i));
                }
            }
        }
        if let Some(h) = hs.iter().filter(|h| matches!(h.0, Handle::Joint(_))).find(|h| near(h.1, HANDLE_PX)) {
            return Some(h.0);
        }
        if let Ok(f) = &m.fit {
            let (o, d) = view.ray(at);
            let best = (0..21)
                .filter_map(|k| scene::ray_shape(o, d, f.shown[k], &f.body.parts[k].shape).map(|t| (k, t)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((k, _)) = best {
                return Some(Handle::Part(k));
            }
        }
        if self.show_bones {
            return (0..m.skel.bones.len())
                .filter_map(|b| view.project(m.skel.bones[b].pose.0 * M).map(|s| (b, s.distance(at))))
                .filter(|x| x.1 <= 6.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|x| Handle::Bone(x.0));
        }
        None
    }

    fn start_drag(&mut self, view: &View, h: Handle, press: Pos2) {
        let Some(m) = &self.cur else { return };
        let Ok(f) = &m.fit else { return };
        let k = match h {
            Handle::Joint(k) | Handle::Radius(k) | Handle::Half(k, _) | Handle::BoxAxis(k, _, _) | Handle::Free(k) | Handle::Axis(k, _) | Handle::Ring(k, _) => k,
            _ => return,
        };
        let g = self.gizmo(view);
        let s = f.shown[k];
        let p = &f.body.parts[k];
        let pe = m.rig.edits.parts[k];
        let moved = matches!(h, Handle::Free(_) | Handle::Axis(..));
        let joint = matches!(h, Handle::Joint(_)) || (moved && g.as_ref().is_some_and(|g| g.joint));
        let (dir, point0, start) = match (h, p.shape, g) {
            (Handle::Joint(_), _, _) => (Vec3::ZERO, f.shown_joints[k], m.rig.edits.joints[k]),
            (Handle::Radius(_), Shape::Capsule { rot, radius, .. }, _) => {
                let axis = s.rotation * rot * Vec3::Y;
                (axis.cross(view.eye - s.translation).normalize_or(Vec3::Z), s.translation, Vec3::splat(radius))
            }
            (Handle::Half(_, sign), Shape::Capsule { rot, half, .. }, _) => (s.rotation * rot * Vec3::Y * sign, s.translation, Vec3::splat(half)),
            (Handle::BoxAxis(_, i, sign), Shape::Box { rot, half }, _) => (s.rotation * rot * Vec3::AXES[i] * sign, s.translation, half),
            (Handle::Free(_) | Handle::Axis(..), _, Some(g)) => {
                let dir = if let Handle::Axis(_, i) = h { g.axes[i] } else { Vec3::ZERO };
                (dir, g.at, if joint { m.rig.edits.joints[k] } else { pe.offset })
            }
            (Handle::Ring(_, i), _, Some(g)) => (if i < 3 { g.axes[i] } else { (view.eye - g.pivot).normalize_or(Vec3::Z) }, g.pivot, pe.turn),
            _ => return,
        };
        let joints0 = f.shown_joints.clone();
        self.drag = Some(Drag { handle: h, press, rig0: m.rig.clone(), shown0: s, dir, point0, start, joint, joints0, ik: self.ik });
    }

    /// The drag at `at`; `snap` (Ctrl) rounds moves to 1 cm, sizes to 5 mm, turns to 5°.
    fn drag_to(&mut self, view: &View, at: Pos2, snap: bool) {
        let (Some(d), Some(m)) = (&self.drag, &mut self.cur) else { return };
        let before = m.rig.clone();
        let total = at - d.press;
        let r = &mut m.rig;
        let snapped = |v: f32, step: f32| if snap { (v / step).round() * step } else { v };
        // The part's frame without its placement (its offset and turn are in it).
        let frame = |k: usize| d.shown0.rotation * scene::from_angles(d.rig0.edits.parts[k].turn).inverse();
        let mut by = None;
        match d.handle {
            Handle::Joint(k) => {
                let want = view.on_plane(at, d.point0);
                let j = &d.joints0;
                // The joints' places without their edits.
                let base = |i: usize| j[i] - d.rig0.edits.joints[i] * M;
                match LIMBS.iter().find(|l| l[1] == k || l[2] == k).filter(|_| d.ik) {
                    Some(&[a, b, c]) if c == k => {
                        let (mid, end) = reach(j[a], j[b], j[c], want);
                        r.edits.joints[b] = (mid - base(b)) / M;
                        r.edits.joints[c] = (end - base(c)) / M;
                    }
                    Some(&[a, b, c]) => r.edits.joints[b] = (swing(j[a], j[b], j[c], want) - base(b)) / M,
                    None => r.edits.joints[k] = d.start + (want - d.point0) / M,
                }
            }
            Handle::Radius(k) => r.edits.parts[k].radius = Some(snapped(d.start.x + view.along(d.point0, d.dir, total), 0.005).max(0.005)),
            Handle::Half(k, _) => r.edits.parts[k].half = Some(snapped(d.start.x + view.along(d.point0, d.dir, total), 0.005).max(0.0)),
            Handle::BoxAxis(k, i, _) => {
                let mut h = d.start;
                h[i] = snapped(h[i] + view.along(d.point0, d.dir, total), 0.005).max(0.01);
                r.edits.parts[k].boxed = Some(h);
            }
            Handle::Free(k) => by = Some((k, view.on_plane(at, d.point0) - d.point0)),
            Handle::Axis(k, _) => by = Some((k, d.dir * snapped(view.along(d.point0, d.dir, total), 0.01))),
            Handle::Ring(k, _) => {
                let hit = |s: Pos2| {
                    let (o, dir) = view.ray(s);
                    let den = dir.dot(d.dir);
                    (den.abs() > 1e-4).then(|| o + dir * ((d.point0 - o).dot(d.dir) / den) - d.point0)
                };
                if let (Some(v0), Some(v1)) = (hit(d.press), hit(at)) {
                    let ang = snapped(v0.cross(v1).dot(d.dir).atan2(v0.dot(v1)), 5f32.to_radians());
                    let turned = Quat::from_axis_angle(d.dir, ang);
                    let t = frame(k);
                    let turn = scene::angles((t.inverse() * turned * d.shown0.rotation).normalize());
                    // Rounded to a hundredth of a degree: the file's text stays short.
                    r.edits.parts[k].turn = (turn * 100.0).round() / 100.0;
                    // About the joint: the part's centre swings round it too.
                    let c0 = d.shown0.translation;
                    if d.point0.distance(c0) > 1e-6 {
                        let c1 = d.point0 + turned * (c0 - d.point0);
                        r.edits.parts[k].offset = d.rig0.edits.parts[k].offset + t.inverse() * (c1 - c0);
                    }
                }
            }
            _ => {}
        }
        if let Some((k, by)) = by {
            if d.joint {
                r.edits.joints[k] = d.start + by / M;
            } else {
                r.edits.parts[k].offset = d.start + frame(k).inverse() * by;
            }
        }
        refit(&self.homer, m);
        if self.symmetry {
            mirror(&self.homer, m, &before);
        }
    }

    /// Esc during a drag: the rig as it was.
    fn cancel_drag(&mut self) {
        if let (Some(d), Some(m)) = (self.drag.take(), &mut self.cur) {
            m.rig = d.rig0;
            refit(&self.homer, m);
        }
    }

    fn end_drag(&mut self) {
        if let (Some(d), Some(m)) = (self.drag.take(), &mut self.cur)
            && m.rig != d.rig0
        {
            m.undo.push(d.rig0);
            m.redo.clear();
        }
    }

    fn view_ui(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let view = View::new(&self.cam, rect);
        let pointer = resp.hover_pos().or(resp.interact_pointer_pos());
        // The camera: the right button orbits, the middle (or Shift + right) pans, the wheel zooms.
        let (shift, ctrl, delta) = ui.input(|i| (i.modifiers.shift, i.modifiers.ctrl, i.pointer.delta()));
        if resp.dragged_by(egui::PointerButton::Secondary) && !shift {
            self.cam.yaw -= delta.x * 0.008;
            self.cam.pitch = (self.cam.pitch + delta.y * 0.008).clamp(-1.5, 1.5);
        }
        if resp.dragged_by(egui::PointerButton::Middle) || (resp.dragged_by(egui::PointerButton::Secondary) && shift) {
            let fwd = (self.cam.target - self.cam.eye()).normalize();
            let right = fwd.cross(Vec3::Z).normalize_or(Vec3::Y);
            let up = right.cross(fwd);
            let s = self.cam.distance * 0.0015;
            self.cam.target += (-right * delta.x + up * delta.y) * s;
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll != 0.0 {
                self.cam.distance = (self.cam.distance * (-scroll * 0.002).exp()).clamp(0.1, 50.0);
            }
        }
        match self.tab {
            Tab::Rig => self.rig_mouse(&view, &resp, pointer, ctrl),
            Tab::Sim => self.sim_mouse(&view, &resp, pointer),
        }
        // The scene.
        let mut sc = Scene { m: view.m, mesh_alpha: self.mesh_alpha, ..Scene::default() };
        let mut grid = Batch { lines: true, ..Batch::default() };
        for i in -10..=10 {
            let (a, c) = (i as f32 * 0.5, [0.3, 0.32, 0.36, 1.0]);
            grid.line(Vec3::new(a, -5.0, 0.0), Vec3::new(a, 5.0, 0.0), c);
            grid.line(Vec3::new(-5.0, a, 0.0), Vec3::new(5.0, a, 0.0), c);
        }
        sc.batches.push(grid);
        let painter = ui.painter_at(rect);
        match self.tab {
            Tab::Rig => self.rig_scene(&view, &mut sc),
            Tab::Sim => self.sim_scene(&mut sc),
        }
        let r = self.renderer.clone();
        let sc = Arc::new(sc);
        painter.add(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, p| {
                r.lock().unwrap_or_else(|e| e.into_inner()).paint(&info, p, &sc);
            })),
        });
        if self.tab == Tab::Rig {
            self.rig_overlay(&view, &painter, pointer);
        }
    }

    fn rig_mouse(&mut self, view: &View, resp: &egui::Response, pointer: Option<Pos2>, snap: bool) {
        if self.drag.is_none() {
            self.hover = pointer.and_then(|p| self.hit(view, p));
        }
        if resp.drag_started_by(egui::PointerButton::Primary)
            && let (Some(h), Some(p)) = (self.hover, resp.interact_pointer_pos())
        {
            if let Handle::Joint(k) = h {
                self.sel = Some(k);
                self.target_joint = true;
            }
            self.start_drag(view, h, p - resp.drag_delta());
        }
        if self.drag.is_some() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.drag_to(view, p, snap);
            }
            if resp.drag_stopped() {
                self.end_drag();
            }
        }
        if resp.clicked_by(egui::PointerButton::Primary) {
            match self.hover {
                Some(Handle::Bone(b)) => {
                    let name = self.cur.as_ref().map(|m| m.skel.bones[b].name.clone());
                    if let Some(k) = self.pick.take() {
                        self.set_bone(k, name);
                        self.sel = Some(k);
                    } else if let Some(m) = &self.cur {
                        let map = m.map();
                        self.sel = (0..21).find(|&k| map[k] == name).or(self.sel);
                    }
                }
                Some(Handle::Part(k)) => {
                    self.sel = Some(k);
                    self.target_joint = false;
                }
                Some(Handle::Joint(k)) => {
                    self.sel = Some(k);
                    self.target_joint = true;
                }
                None => {
                    self.sel = None;
                    self.pick = None;
                }
                _ => {}
            }
        }
    }

    fn rig_scene(&mut self, _view: &View, sc: &mut Scene) {
        let Some(m) = &self.cur else { return };
        sc.mesh = m.batch.clone();
        sc.mesh_version = m.version;
        if let Ok(f) = &m.fit {
            for k in 0..21 {
                let hot = self.sel == Some(k) || self.hover == Some(Handle::Part(k));
                let mut c = scene::part_color(k, self.parts_alpha);
                if hot {
                    c = [c[0] * 0.5 + 0.5, c[1] * 0.5 + 0.5, c[2] * 0.5 + 0.5, c[3]];
                }
                let shape = f.body.parts[k].shape;
                let (pts, tris) = self.hulls.get(&shape);
                let mut b = Batch::default();
                b.hull(pts, tris, f.shown[k], c);
                sc.batches.push(b);
            }
            // Where NM's head looks: an arrow out of its face (a capsule shows no face of its own).
            let shape = f.body.parts[12].shape;
            let reach = match shape {
                Shape::Capsule { radius, .. } => radius,
                Shape::Box { half, .. } => half.max_element(),
            };
            let (c, way) = (f.shown[12].translation, snap::face(f));
            let up = (f.shown[12].rotation * shape_rot(&shape) * Vec3::Y).reject_from(way).normalize_or_zero();
            let tip = c + way * (reach + 0.1);
            let colour = [1.0, 0.95, 0.3, 1.0];
            let mut b = Batch { lines: true, on_top: true, ..Batch::default() };
            b.line(c + way * reach, tip, colour);
            for side in [up, way.cross(up)] {
                b.line(tip, tip - way * 0.03 + side * 0.02, colour);
                b.line(tip, tip - way * 0.03 - side * 0.02, colour);
            }
            sc.batches.push(b);
        }
        if self.show_bones || self.pick.is_some() {
            let map = m.map();
            let mut b = Batch { lines: true, on_top: true, ..Batch::default() };
            for (i, bone) in m.skel.bones.iter().enumerate() {
                if let Some(p) = bone.parent {
                    let k = map.iter().position(|x| x.as_deref() == Some(bone.name.as_str()));
                    let c = k.map_or([0.85, 0.85, 0.9, 1.0], |k| scene::part_color(k, 1.0));
                    let _ = i;
                    b.line(m.skel.bones[p].pose.0 * M, bone.pose.0 * M, c);
                }
            }
            sc.batches.push(b);
        }
    }

    fn rig_overlay(&self, view: &View, painter: &egui::Painter, pointer: Option<Pos2>) {
        let Some(m) = &self.cur else { return };
        let text = |p: Pos2, s: &str, c: Color32| {
            painter.text(p + egui::vec2(9.0, -9.0), egui::Align2::LEFT_BOTTOM, s, egui::FontId::proportional(13.0), c);
        };
        if let Some(k) = self.pick {
            painter.text(view.rect.left_top() + egui::vec2(10.0, 10.0), egui::Align2::LEFT_TOP, format!("click the bone for {} (Esc: cancel)", PARTS[k]), egui::FontId::proportional(16.0), Color32::YELLOW);
            for b in &m.skel.bones {
                if let Some(s) = view.project(b.pose.0 * M) {
                    painter.circle_filled(s, 3.0, Color32::from_gray(220));
                }
            }
            if let Some(Handle::Bone(b)) = self.hover
                && let Some(s) = view.project(m.skel.bones[b].pose.0 * M)
            {
                painter.circle_stroke(s, 7.0, Stroke::new(2.0_f32, Color32::YELLOW));
                text(s, &m.skel.bones[b].name, Color32::YELLOW);
            }
            return;
        }
        let Ok(f) = &m.fit else { return };
        let hs = self.handles(view);
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hover);
        let gizmo = self.gizmo(view);
        // The limbs' mirror images: the selected part's other side, when symmetric.
        if self.symmetry
            && let Some(k) = self.sel
            && let Some(o) = PAIRS.iter().find_map(|&(a, b)| if a == k { Some(b) } else if b == k { Some(a) } else { None })
            && let Some(s) = view.project(f.shown[o].translation)
        {
            painter.circle_stroke(s, 9.0, Stroke::new(1.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 140)));
        }
        if let Some(g) = &gizmo
            && self.tool.turns()
        {
            for i in 0..4 {
                let hot = active == Some(Handle::Ring(g.k, i));
                let c = if hot { HOT } else if i < 3 { AXIS_COLORS[i] } else { Color32::from_gray(190) };
                painter.add(egui::Shape::line(Self::ring(view, g, i), Stroke::new(if hot { 3.0_f32 } else { 1.5 }, c)));
            }
        }
        if let Some(g) = &gizmo
            && self.tool.moves()
            && let Some(c) = view.project(g.at)
        {
            for i in 0..3 {
                let Some(t) = view.project(g.at + g.axes[i] * g.arrow) else { continue };
                if t.distance(c) < 8.0 {
                    continue;
                }
                let hot = active == Some(Handle::Axis(g.k, i));
                let col = if hot { HOT } else { AXIS_COLORS[i] };
                painter.line_segment([c, t], Stroke::new(if hot { 4.0_f32 } else { 2.5 }, col));
                let (dir, side) = ((t - c).normalized(), (t - c).normalized().rot90());
                painter.add(egui::Shape::convex_polygon(vec![t + dir * 11.0, t + side * 5.0, t - side * 5.0], col, Stroke::NONE));
            }
        }
        for (h, p) in &hs {
            let Some(s) = view.project(*p) else { continue };
            let hot = active == Some(*h);
            match h {
                Handle::Axis(..) => {}
                Handle::Joint(k) => {
                    let sel = self.sel == Some(*k) && (self.target_joint || !self.tool.moves());
                    let c = if SPREAD.contains(k) { Color32::from_rgb(200, 160, 255) } else { Color32::from_rgb(255, 210, 60) };
                    painter.circle_filled(s, if sel || hot { HANDLE_PX } else { 4.5 }, c);
                    painter.circle_stroke(s, if sel || hot { HANDLE_PX } else { 4.5 }, Stroke::new(1.0_f32, Color32::BLACK));
                    if hot {
                        text(s, PARTS[*k], c);
                    }
                }
                Handle::Free(_) => {
                    let r = egui::Rect::from_center_size(s, egui::vec2(12.0, 12.0));
                    painter.rect_stroke(r, 0.0, Stroke::new(if hot { 3.0_f32 } else { 2.0 }, if hot { HOT } else { Color32::WHITE }), egui::StrokeKind::Middle);
                }
                _ => {
                    // A scale handle: a square on the shape, tied to its centre.
                    let c = if hot { HOT } else { Color32::from_rgb(80, 220, 255) };
                    if let Some(c0) = view.project(f.shown[self.sel.unwrap_or(0)].translation) {
                        painter.line_segment([c0, s], Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(80, 220, 255, 120)));
                    }
                    painter.rect_filled(egui::Rect::from_center_size(s, egui::vec2(9.0, 9.0) * if hot { 1.3 } else { 1.0 }), 0.0, c);
                }
            }
        }
        if let (Some(Handle::Bone(b)), Some(p)) = (self.hover, pointer) {
            text(p, &m.skel.bones[b].name, Color32::WHITE);
        }
        if let (Some(Handle::Part(k)), Some(p)) = (self.hover, pointer) {
            text(p, PARTS[k], Color32::WHITE);
        }
    }

    fn sim_mouse(&mut self, view: &View, resp: &egui::Response, _pointer: Option<Pos2>) {
        let Some(s) = &mut self.sim else { return };
        if resp.drag_started_by(egui::PointerButton::Primary)
            && let (Some(p), Some(states), Some(body)) = (resp.interact_pointer_pos(), s.states(), s.body())
        {
            let (o, d) = view.ray(p - resp.drag_delta());
            let best = states
                .iter()
                .enumerate()
                .filter_map(|(k, st)| scene::ray_shape(o, d, Transform::from_translation(st.pos).with_rotation(st.rot), &body.parts[k].shape).map(|t| (k, t)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((k, t)) = best {
                let hit = o + d * t;
                s.grab = Some(sim::Grab { part: k, local: states[k].rot.inverse() * (hit - states[k].pos), target: hit });
            }
        }
        if let (Some(g), Some(p)) = (&mut s.grab, resp.interact_pointer_pos())
            && resp.dragged_by(egui::PointerButton::Primary)
        {
            g.target = view.on_plane(p, g.target);
        }
        if resp.drag_stopped() {
            s.grab = None;
        }
    }

    fn sim_scene(&mut self, sc: &mut Scene) {
        let Some(s) = &self.sim else { return };
        let (Some(states), Some(body)) = (s.states(), s.body()) else { return };
        if states.len() != body.parts.len() {
            return;
        }
        let parts: Vec<(Vec3, Quat)> = states.iter().map(|st| (st.pos, st.rot)).collect();
        // The model's mesh on its fitted body (not on Homer's).
        if let Some(m) = self.cur.as_ref().filter(|_| !self.sim_homer)
            && let (Some(me), Ok(f)) = (&m.mesh, &m.fit)
            && self.mesh_alpha > 0.0
        {
            let world = scene::pose_bones(&m.skel, f, &parts);
            let mut b = scene::skinned_batch(me, &m.skel, &world, [0.78, 0.76, 0.72, self.mesh_alpha.min(1.0)]);
            if self.mesh_alpha >= 1.0 {
                b.verts.iter_mut().for_each(|v| v.color[3] = 1.0);
            }
            sc.batches.push(b);
        }
        if self.parts_alpha > 0.0 {
            for (k, st) in states.iter().enumerate() {
                let shape = body.parts[k].shape;
                let (pts, tris) = self.hulls.get(&shape);
                let mut b = Batch::default();
                b.hull(pts, tris, Transform::from_translation(st.pos).with_rotation(st.rot), scene::part_color(k, self.parts_alpha));
                sc.batches.push(b);
            }
        }
        if let Some(g) = &s.grab {
            let st = states[g.part];
            let mut b = Batch { lines: true, on_top: true, ..Batch::default() };
            b.line(st.pos + st.rot * g.local, g.target, [1.0, 1.0, 0.3, 1.0]);
            sc.batches.push(b);
        }
    }

    // ---- the panels ---------------------------------------------------------------------------------------------

    fn models_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Models");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("filter").desired_width(170.0));
            ui.checkbox(&mut self.workshop_only, "Workshop");
        });
        let words: Vec<String> = self.filter.to_lowercase().split_whitespace().map(str::to_string).collect();
        let list: Vec<(String, usize)> = self
            .models
            .iter()
            .filter(|(p, s)| words.iter().all(|w| p.contains(w.as_str())) && (!self.workshop_only || self.fs.sources[*s].name.starts_with("workshop")))
            .cloned()
            .collect();
        ui.horizontal(|ui| {
            ui.label(format!("{} shown", list.len()));
            if ui.button("Classify shown").on_hover_text("reads each shown model's skeleton (up to 400) and tells its rig").clicked() {
                let todo: Vec<String> = list.iter().filter(|(p, _)| !self.classes.contains_key(p)).take(400).map(|x| x.0.clone()).collect();
                for p in &todo {
                    let c = self.fs.read(p).and_then(|b| read_mdl(&b)).and_then(|s| {
                        let at: Vec<Vec3> = s.bones.iter().map(|b| b.bind.0).collect();
                        rigs::classify(&s, None, &at).map(|c| if c.complete() { c.rig } else { format!("! {}", c.rig) })
                    });
                    self.classes.insert(p.clone(), c.unwrap_or_else(|e| format!("? {e}")));
                }
            }
        });
        let open = self.cur.as_ref().map(|m| m.path.clone());
        let mut clicked = None;
        egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, 18.0, list.len(), |ui, range| {
            for (p, s) in &list[range] {
                let tag = self.classes.get(p).map(|c| format!("  [{c}]")).unwrap_or_default();
                let r = ui.selectable_label(open.as_deref() == Some(p.as_str()), format!("{}{tag}", p.trim_start_matches("models/")));
                if r.clicked() {
                    clicked = Some(p.clone());
                }
                r.on_hover_text(&self.fs.sources[*s].name);
            }
        });
        if let Some(p) = clicked {
            self.open(&p);
        }
    }

    fn rig_ui(&mut self, ui: &mut egui::Ui) {
        let Some(m) = &self.cur else {
            ui.label("Open a model on the left.");
            return;
        };
        ui.heading(m.path.trim_start_matches("models/"));
        ui.label(egui::RichText::new(&m.source).weak());
        ui.label(format!("own rig: {}", m.auto_rig));
        match &m.fit {
            Ok(f) => {
                ui.label(format!("fitted: {}, {:.2} m, {:.1} kg", f.rig, f.height, f.mass));
                for w in &f.warnings {
                    ui.colored_label(Color32::from_rgb(240, 190, 80), w);
                }
            }
            Err(e) => {
                ui.colored_label(Color32::from_rgb(255, 110, 110), e);
            }
        }
        for p in &m.problems {
            ui.colored_label(Color32::from_rgb(240, 190, 80), p);
        }
        ui.separator();
        // The file.
        let file = m.file(&self.opts.gmod);
        let dirty = m.saved.as_ref() != Some(&m.rig);
        ui.label(format!("data/dmr/skeletons/{}{}", rigfile::file_name(&m.path), if m.saved.is_none() { " (none yet)" } else if dirty { " (changed)" } else { "" }));
        ui.horizontal_wrapped(|ui| {
            if ui.button("Save").on_hover_text("Ctrl+S; the game reloads the file and refits the model's ragdolls with NM").clicked() {
                self.save();
            }
            if ui.add_enabled(self.cur.as_ref().is_some_and(|m| m.saved.is_some()), egui::Button::new("Reload file")).clicked()
                && let Ok(t) = std::fs::read_to_string(&file)
            {
                self.take_rig(&t, "the file");
            }
            if ui.button("Copy JSON").on_hover_text("the rig's text, to share").clicked()
                && let Some(m) = &self.cur
            {
                ui.ctx().copy_text(rigfile::encode(&m.rig));
                self.status = "the rig's JSON is on the clipboard".into();
            }
            if ui.button("Model's own").on_hover_text("no mapping, no edits (Save writes an empty rig; delete the file to drop it)").clicked() {
                self.edit(|r| {
                    r.bones.clear();
                    r.edits = rigs::Edits::default();
                });
            }
            let (u, r) = self.cur.as_ref().map_or((false, false), |m| (!m.undo.is_empty(), !m.redo.is_empty()));
            if ui.add_enabled(u, egui::Button::new("Undo")).clicked() {
                self.undo(false);
            }
            if ui.add_enabled(r, egui::Button::new("Redo")).clicked() {
                self.undo(true);
            }
        });
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.import).hint_text("a rig file to import").desired_width(220.0));
            if ui.button("Import").clicked() {
                let p = self.import.trim().trim_matches('"').to_string();
                match std::fs::read_to_string(&p) {
                    Ok(t) => self.take_rig(&t, &p),
                    Err(e) => self.status = format!("{p}: {e}"),
                }
            }
        });
        ui.collapsing("Paste JSON", |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.paste).desired_rows(4).desired_width(f32::INFINITY));
            if ui.button("Use it").clicked() {
                let t = self.paste.clone();
                self.take_rig(&t, "the pasted text");
            }
        });
        ui.separator();
        self.parts_ui(ui);
    }

    fn parts_ui(&mut self, ui: &mut egui::Ui) {
        let Some(m) = &self.cur else { return };
        let map = m.map();
        let bones: Vec<String> = m.skel.bones.iter().map(|b| b.name.clone()).collect();
        let mut set: Option<(usize, Option<String>)> = None;
        let mut pick = None;
        egui::Grid::new("parts").num_columns(3).striped(true).show(ui, |ui| {
            for k in 0..21 {
                let sel = self.sel == Some(k);
                let name = egui::RichText::new(PARTS[k]).color(Color32::from_rgb(
                    (scene::part_color(k, 1.0)[0] * 255.0) as u8,
                    (scene::part_color(k, 1.0)[1] * 255.0) as u8,
                    (scene::part_color(k, 1.0)[2] * 255.0) as u8,
                ));
                if ui.selectable_label(sel, name).clicked() {
                    self.sel = Some(k);
                }
                if SPREAD.contains(&k) {
                    ui.label(egui::RichText::new("spread").weak());
                    ui.label("");
                } else {
                    let cur = map[k].clone().unwrap_or_else(|| "—".into());
                    egui::ComboBox::from_id_salt(("bone", k)).selected_text(&cur).width(170.0).height(400.0).show_ui(ui, |ui| {
                        if ui.selectable_label(map[k].is_none(), "—").clicked() {
                            set = Some((k, None));
                        }
                        for b in &bones {
                            if ui.selectable_label(map[k].as_deref() == Some(b.as_str()), b).clicked() {
                                set = Some((k, Some(b.clone())));
                            }
                        }
                    });
                    if ui.small_button("pick").on_hover_text("click the bone in the view").clicked() {
                        pick = Some(k);
                    }
                }
                ui.end_row();
            }
        });
        if let Some((k, b)) = set {
            self.set_bone(k, b);
        }
        if pick.is_some() {
            self.pick = pick;
            self.show_bones = true;
        }
        ui.separator();
        let Some(k) = self.sel else {
            ui.label("Click a part (or a joint) to edit it.");
            return;
        };
        let Some(m) = &self.cur else { return };
        let Ok(f) = &m.fit else { return };
        let mut pe = m.rig.edits.parts[k];
        let mut joint = m.rig.edits.joints[k];
        let p = &f.body.parts[k];
        ui.strong(PARTS[k]);
        let mut changed = false;
        let row = |ui: &mut egui::Ui, label: &str, v: &mut f32, speed: f64, unit: &str| -> bool {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(v).speed(speed).max_decimals(4).suffix(unit)).changed()
            })
            .inner
        };
        ui.label("joint moved (units, the model's space)");
        ui.horizontal(|ui| {
            for i in 0..3 {
                changed |= ui.add(egui::DragValue::new(&mut joint[i]).speed(0.05).max_decimals(3)).changed();
            }
        });
        match p.shape {
            Shape::Capsule { radius, half, .. } => {
                let (mut r, mut h) = (pe.radius.unwrap_or(radius), pe.half.unwrap_or(half));
                changed |= row(ui, "radius", &mut r, 0.001, " m");
                changed |= row(ui, "half length", &mut h, 0.001, " m");
                if (r - radius).abs() > 1e-6 {
                    pe.radius = Some(r.max(0.005));
                }
                if (h - half).abs() > 1e-6 {
                    pe.half = Some(h.max(0.0));
                }
            }
            Shape::Box { half, .. } => {
                let mut b = pe.boxed.unwrap_or(half);
                ui.label("box half extents (m)");
                ui.horizontal(|ui| {
                    for i in 0..3 {
                        changed |= ui.add(egui::DragValue::new(&mut b[i]).speed(0.001).max_decimals(4)).changed();
                    }
                });
                if b != half {
                    pe.boxed = Some(b.max(Vec3::splat(0.01)));
                }
            }
        }
        let mut mass = pe.mass.unwrap_or(p.mass);
        changed |= row(ui, "mass", &mut mass, 0.05, " kg");
        if (mass - p.mass).abs() > 1e-4 {
            pe.mass = Some(mass.max(0.05));
        }
        ui.label("on the model's bone: moved (m), turned (pitch, yaw, roll °)");
        ui.horizontal(|ui| {
            for i in 0..3 {
                changed |= ui.add(egui::DragValue::new(&mut pe.offset[i]).speed(0.001).max_decimals(4)).changed();
            }
        });
        ui.horizontal(|ui| {
            for i in 0..3 {
                changed |= ui.add(egui::DragValue::new(&mut pe.turn[i]).speed(0.5).max_decimals(2)).changed();
            }
        });
        let reset = ui.button("Reset this part").clicked();
        if changed || reset {
            // A drag of a number is one undo step: the first change of a drag pushes.
            let dragging = ui.ctx().dragged_id().is_some() && !reset;
            let Some(m) = &mut self.cur else { return };
            let before = m.rig.clone();
            if reset {
                m.rig.edits.parts[k] = rigs::PartEdit::default();
                m.rig.edits.joints[k] = Vec3::ZERO;
            } else {
                m.rig.edits.parts[k] = pe;
                m.rig.edits.joints[k] = joint;
            }
            let rig_before = before.clone();
            if m.rig != before {
                if !(dragging && self.number_drag) {
                    m.undo.push(before);
                }
                m.redo.clear();
            }
            self.number_drag = dragging;
            refit(&self.homer, m);
            if self.symmetry {
                mirror(&self.homer, m, &rig_before);
            }
        }
    }

    /// Under NM: the room's shot, the Playroom's buttons and a message line, as the game sends them.
    fn nm_ui(&mut self, ui: &mut egui::Ui) {
        use dms_vphysics::follow::shot::WEAPONS;
        let parts = delirium_data::nm_asset::PARTS;
        ui.horizontal_wrapped(|ui| {
            let shoot = ui.button("Shoot").on_hover_text("the room's shot (the Playroom's shot button): the weapon's damage on his health, its impulse and the ShotTask with its taskparams set, as the game's bullets").clicked();
            ui.checkbox(&mut self.shot_at.back, "from behind");
            egui::ComboBox::from_id_salt("weapon").selected_text(WEAPONS[self.shot_at.weapon].0).show_ui(ui, |ui| {
                for (i, w) in WEAPONS.iter().enumerate() {
                    ui.selectable_value(&mut self.shot_at.weapon, i, w.0);
                }
            });
            egui::ComboBox::from_id_salt("shot part").selected_text(parts.get(self.shot_at.part).copied().unwrap_or("?")).show_ui(ui, |ui| {
                for (k, p) in parts.iter().enumerate().take(dms_vphysics::follow::PARTS) {
                    ui.selectable_value(&mut self.shot_at.part, k, *p);
                }
            });
            if shoot && let Some(s) = &mut self.sim {
                s.shoot(self.shot_at);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for b in ["balance", "windmill", "pedal", "writhe", "rollup", "catchfall", "highfall", "stairs", "relax", "stayup", "foetal", "STOP all"] {
                let lines = dms_vphysics::follow::button_lines(b).unwrap_or_default().join("; ");
                if ui.small_button(b).on_hover_text(format!("the Playroom's button: {lines}")).clicked()
                    && let Some(s) = &mut self.sim
                {
                    s.send(b);
                }
            }
        });
        ui.horizontal(|ui| {
            let edit = ui.add(egui::TextEdit::singleline(&mut self.nm_line).hint_text("bodyBalance start=true  (message param=value ...)").desired_width(ui.available_width() - 60.0));
            let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            // Up: the last line sent again.
            if edit.has_focus() && ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) && let Some(l) = self.nm_sent.last() {
                self.nm_line = l.clone();
            }
            if (ui.button("Send").clicked() || enter) && !self.nm_line.trim().is_empty() {
                let line = self.nm_line.trim().to_string();
                if let Some(s) = &mut self.sim {
                    s.send(&line);
                }
                self.nm_sent.retain(|l| l != &line);
                self.nm_sent.push(line);
                self.nm_line.clear();
                if enter {
                    edit.request_focus();
                }
            }
        });
    }

    fn sim_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Simulation on the fork");
        ui.label("The fitted body made by VPhysics-Box3D as the module makes it (IPhysicsNM: CreateBody, CreateJoint, the bind, NM's collision pairs); NM on it as the module's agents drive one.");
        self.start_sim();
        if let Some(e) = &self.sim_err {
            ui.colored_label(Color32::from_rgb(255, 110, 110), e);
        }
        let Some(s) = &self.sim else { return };
        ui.label(egui::RichText::new(format!("{}  ({} surface properties)", s.dll.display(), s.surfaces)).weak());
        if ui.checkbox(&mut self.sim_homer, "Homer's own body (golden's), not the fit").changed()
            && let Some(s) = &mut self.sim
        {
            s.clear();
        }
        ui.separator();
        ui.label(egui::RichText::new("NM (Delirium, FOLLOW)").strong());
        ui.horizontal_wrapped(|ui| {
            if ui.button("NM: bodyBalance").on_hover_text("stands the body and puts the Playroom's agent on it, its balance button pressed (bodyBalance start=true)").clicked() {
                self.sim_nm("balance");
            }
            let on = self.sim.as_ref().is_some_and(|s| s.with_nm(|_| ()).is_some());
            if ui.add_enabled(on, egui::Button::new("Push")).on_hover_text("the Playroom's poke: an impulse into the spine (part 9), away from the view").clicked() {
                self.sim_push();
            }
            ui.add(egui::DragValue::new(&mut self.push).range(1.0..=200.0).speed(0.5).suffix(" N s"));
            if ui.add_enabled(on, egui::Button::new("NM off")).clicked()
                && let Some(s) = &mut self.sim
            {
                s.nm_off();
            }
        });
        let on = self.sim.as_ref().is_some_and(|s| s.with_nm(|_| ()).is_some());
        ui.add_enabled_ui(on, |ui| self.nm_ui(ui));
        if let Some(s) = &self.sim {
            s.with_nm(|a| {
                let state = match &a.failed {
                    Some(e) => format!("NM came off: {e}"),
                    None if a.agent.fallen => "fallen".into(),
                    None if a.agent.balancing() => "balancing".into(),
                    None => "not balancing".into(),
                };
                ui.label(format!("{}: {state}; {} NM steps, {:.2} ms a step", a.button, a.steps, a.cost));
                let shot = if a.shots.task() { ", the shot task running" } else { "" };
                ui.label(format!("health {:.0}{}{shot}", a.shots.health, if a.shots.dead { " (dead: limp)" } else { "" }));
                for l in &a.log {
                    ui.label(egui::RichText::new(l).weak().small());
                }
            });
        }
        ui.separator();
        ui.label(egui::RichText::new("The body alone").strong());
        ui.horizontal_wrapped(|ui| {
            if ui.button("Stand").clicked() {
                self.sim_place(0.0, 0.0, Vec3::ZERO, Vec3::ZERO);
            }
            if ui.button("Drop").on_hover_text("from 1 m, leaning").clicked() {
                self.sim_drop();
            }
            if ui.button("Throw").clicked() {
                self.sim_place(0.2, 0.0, Vec3::ZERO, Vec3::ZERO);
                if let Some(s) = &mut self.sim {
                    s.kick(Vec3::new(4.0, 1.0, 3.5), Vec3::new(1.5, -4.0, 2.0));
                }
            }
            if ui.button("Kick").on_hover_text("the body as it is, every part given 3 m/s sideways and up, and a spin").clicked()
                && let Some(s) = &mut self.sim
            {
                s.kick(Vec3::new(0.0, 3.0, 2.0), Vec3::new(0.0, 0.0, 3.0));
            }
            if ui.button("Clear").clicked()
                && let Some(s) = &mut self.sim
            {
                s.clear();
            }
        });
        let Some(s) = &mut self.sim else { return };
        ui.horizontal(|ui| {
            ui.checkbox(&mut s.paused, "pause");
            ui.add(egui::Slider::new(&mut s.speed, 0.05..=2.0).text("speed"));
        });
        let mut g = s.nm_gravity;
        ui.horizontal(|ui| {
            ui.radio_value(&mut g, true, "NM's gravity (9.8 m/s², as under NM)").on_hover_text("under NM: Source's gravity and the module's compensating force (docs/adr/0007), else the environment's own");
            ui.radio_value(&mut g, false, "Source's (600 u/s²)");
        });
        if g != s.nm_gravity {
            s.set_gravity(g);
        }
        ui.label(format!("t = {:.2} s, {} steps of {} s, {:.2} ms a step", s.time, s.steps, sim::TIMESTEP, s.step_ms));
        if let Some(st) = s.states() {
            let low = st.iter().map(|p| p.pos.z).fold(f32::MAX, f32::min);
            ui.label(format!("pelvis at {:.2} m, the lowest part's centre {:.2} m", st[0].pos.z, low));
        }
        if !s.has_body() {
            ui.label("NM: bodyBalance, Stand, Drop or Throw puts the fitted body in; drag a part with the left button (under NM: the Playroom's grab).");
        }
    }
}

fn seg_dist(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let (ab, ap) = (b - a, p - a);
    let t = (ap.dot(ab) / ab.length_sq().max(1e-6)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let now = std::time::Instant::now();
        let dt = (now - self.last).as_secs_f32();
        self.last = now;
        self.frames += 1;
        // Keys.
        let (undo, redo, save, esc) = ctx.input_mut(|i| {
            let redo = i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y) || i.consume_key(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z);
            (i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z), redo, i.consume_key(egui::Modifiers::COMMAND, egui::Key::S), i.key_pressed(egui::Key::Escape))
        });
        if undo {
            self.undo(false);
        }
        if redo {
            self.undo(true);
        }
        if save {
            self.save();
        }
        if esc {
            self.pick = None;
            self.cancel_drag();
        }
        // The tools' keys (not while typing, not with Ctrl).
        if self.tab == Tab::Rig && !ctx.wants_keyboard_input() {
            ctx.input(|i| {
                if i.modifiers.is_none() {
                    if let Some(t) = Tool::ALL.iter().find(|t| i.key_pressed(t.2)) {
                        self.tool = t.0;
                    }
                    if i.key_pressed(egui::Key::M) {
                        self.symmetry = !self.symmetry;
                    }
                    if i.key_pressed(egui::Key::I) {
                        self.ik = !self.ik;
                    }
                }
            });
            let (one, all) = ctx.input(|i| (i.modifiers.is_none() && i.key_pressed(egui::Key::B), i.modifiers.matches_exact(egui::Modifiers::SHIFT) && i.key_pressed(egui::Key::B)));
            if one && let Some(k) = self.sel {
                self.snap(vec![k]);
            }
            if all {
                self.snap((0..21).collect());
            }
        }
        // The physics: real time, or the screenshot's steps at the game's pace a frame.
        if self.tab == Tab::Sim
            && let Some(s) = &mut self.sim
        {
            let want = self.shot.as_ref().map(|x| x.1).unwrap_or(0);
            if want > 0 {
                let dir = (self.cam.target - self.cam.eye()).with_z(0.0).normalize_or(Vec3::Y);
                while s.steps < want {
                    if s.steps == want / 2 {
                        if let Some(weapon) = self.opts.shoot {
                            s.shoot(sim::ShotAt { weapon, ..Default::default() });
                        } else if let Some(push) = self.opts.push {
                            s.poke(dms_vphysics::follow::POKE_PART, dir, push);
                        }
                    }
                    s.run(sim::TIMESTEP);
                    if s.paused || !s.has_body() {
                        break;
                    }
                }
            } else {
                s.run(dt);
            }
        }
        egui::TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Rig, "Rig");
                ui.selectable_value(&mut self.tab, Tab::Sim, "Simulate");
                ui.separator();
                ui.add(egui::Slider::new(&mut self.mesh_alpha, 0.0..=1.0).text("mesh"));
                ui.add(egui::Slider::new(&mut self.parts_alpha, 0.0..=1.0).text("parts"));
                ui.checkbox(&mut self.show_bones, "bones");
                ui.separator();
                ui.label(egui::RichText::new("left: select / drag · Ctrl: snap · right: orbit · middle or Shift+right: pan · wheel: zoom · Ctrl+Z/Y, Ctrl+S").weak());
            });
            if self.tab == Tab::Rig {
                ui.horizontal(|ui| {
                    for (t, name, key, hint) in Tool::ALL {
                        ui.selectable_value(&mut self.tool, t, format!("{name} ({})", key.name())).on_hover_text(hint);
                    }
                    ui.separator();
                    ui.label("axes");
                    ui.selectable_value(&mut self.space, Space::Part, "Part").on_hover_text("the part's own: a capsule's green axis is its length, along its bone");
                    ui.selectable_value(&mut self.space, Space::Bone, "Bone").on_hover_text("the model's bone mapped to the part");
                    ui.selectable_value(&mut self.space, Space::World, "World");
                    ui.separator();
                    ui.label("turn about");
                    ui.selectable_value(&mut self.pivot_joint, false, "centre");
                    ui.selectable_value(&mut self.pivot_joint, true, "joint");
                    ui.separator();
                    ui.toggle_value(&mut self.symmetry, "Symmetry (M)").on_hover_text("a limb's edit is mirrored on the other side, across the model's plane between its hips");
                    ui.toggle_value(&mut self.ik, "IK (I)").on_hover_text("dragging a wrist or ankle joint carries the elbow or knee, the limb's lengths kept; an elbow or knee turns about the limb's line");
                    ui.separator();
                    const SNAP: &str = "lay the part on the model: along its bone from its joint to the next (the pelvis, feet, hands and head along their mesh), as thick and long as the mesh its bones carry";
                    if ui.add_enabled(self.sel.is_some(), egui::Button::new("Snap to bones (B)")).on_hover_text(SNAP).clicked()
                        && let Some(k) = self.sel
                    {
                        self.snap(vec![k]);
                    }
                    if ui.button("Snap all (Shift+B)").on_hover_text(SNAP).clicked() {
                        self.snap((0..21).collect());
                    }
                });
            }
        });
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.label(&self.status);
        });
        egui::SidePanel::left("models").default_width(330.0).show(ctx, |ui| self.models_ui(ui));
        egui::SidePanel::right("rig").default_width(390.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| match self.tab {
                Tab::Rig => self.rig_ui(ui),
                Tab::Sim => self.sim_ui(ui),
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| self.view_ui(ui));
        // --screenshot: once drawn (and simulated), the window to a PNG, then quit.
        if let Some((path, _)) = self.shot.clone() {
            let shot = ctx.input(|i| {
                i.raw.events.iter().find_map(|e| match e {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(img) = shot {
                let r = image::save_buffer(&path, img.as_raw(), img.width() as u32, img.height() as u32, image::ExtendedColorType::Rgba8);
                println!("dms-rig: {} {}", path.display(), r.map_or_else(|e| format!("failed: {e}"), |_| "saved".into()));
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if self.frames > 8 && !self.shot_sent {
                self.shot_sent = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
            }
        }
        ctx.request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_keeps_lengths() {
        let (root, mid, end) = (Vec3::ZERO, Vec3::new(0.3, 0.0, -0.05), Vec3::new(0.55, 0.0, 0.0));
        for want in [Vec3::new(0.2, 0.3, 0.1), Vec3::new(2.0, 0.0, 0.0), Vec3::new(0.01, 0.0, 0.0)] {
            let (m, e) = reach(root, mid, end, want);
            assert!((m.distance(root) - mid.distance(root)).abs() < 1e-3);
            assert!((e.distance(m) - end.distance(mid)).abs() < 1e-3);
        }
        // Within reach the end gets there, bent the way it was (down).
        let (m, e) = reach(root, mid, end, Vec3::new(0.4, 0.1, 0.0));
        assert!(e.distance(Vec3::new(0.4, 0.1, 0.0)) < 1e-4);
        assert!(m.z < 0.0);
    }

    #[test]
    fn swing_keeps_ends() {
        let (root, mid, end) = (Vec3::ZERO, Vec3::new(0.3, 0.0, -0.05), Vec3::new(0.55, 0.0, 0.0));
        let m = swing(root, mid, end, Vec3::new(0.3, 0.5, 0.0));
        assert!((m.distance(root) - mid.distance(root)).abs() < 1e-4);
        assert!((end.distance(m) - end.distance(mid)).abs() < 1e-4);
        assert!(m.y > 0.04);
    }
}
