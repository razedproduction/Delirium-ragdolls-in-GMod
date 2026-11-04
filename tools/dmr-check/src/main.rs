//! dmr-check: bodies fitted to GMod's models against NM on the bench (ROADMAP M10, M11).
//!
//! There's no Playroom to hold a fitted body to (it can't load a fitted build, ADR 0025): `fit` holds FOLLOW on a
//! fitted native body to the same build in LEAD (the crate as the Playroom runs it), `body` holds the native body to
//! the build it was made from. Records go to `fit/out/` (made from the player's game files: never committed, ADR 0006).
//! See `fit/README.md`.

mod body;

use std::path::{Path, PathBuf};

use delirium_physics as sdk;

const USAGE: &str = "\
usage: dmr-check [--game <GTA IV dir>] <command>
  fit [--model <mdl>] [names]
               a body fitted to a GMod model (default male_07; stand, push, shot, catchfall unless named) on the bench
               in FOLLOW on its native body, against the same build in LEAD (the crate as the Playroom runs it)
               -> fit/out/<model>/<name>.{lead,follow}.rec, held to the scenario's `#! tol`
  body [<mdl>] a body fitted to a GMod model (default male_07): its build against the SDK's own fit, then on the bench
               its native body (no NM) against it, then dropped
  links        the links DMS derives from the idle clip -> fit/out/homer.links (body makes them if they're missing)
The game's folder: --game (remembered in fit/game_dir.txt), or the DMR_GAME_DIR environment variable.";

/// male_07, the model the fit is checked on by default.
const DEFAULT_MODEL: &str = "models/humans/group01/male_07.mdl";

struct Ctx {
    repo: PathBuf,
    out: PathBuf,
    game: Option<PathBuf>,
}

impl Ctx {
    fn game(&self) -> Result<&Path, String> {
        self.game.as_deref().ok_or_else(|| {
            "no game folder: pass --game <your GTA IV folder> (PC: with GTAIV.exe; Xbox 360: with default.xex)".into()
        })
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut take = |flag: &str| {
        args.iter().position(|a| a == flag).map(|i| {
            let v = args.get(i + 1).cloned().unwrap_or_default();
            args.drain(i..(i + 2).min(args.len()));
            v
        })
    };
    let repo = dmr_ctl::repo();
    let remembered = repo.join("fit").join("game_dir.txt");
    let game = match take("--game") {
        Some(g) => {
            let _ = std::fs::write(&remembered, &g);
            Some(PathBuf::from(g))
        }
        None => std::env::var_os("DMR_GAME_DIR")
            .map(PathBuf::from)
            .or_else(|| std::fs::read_to_string(&remembered).ok().map(|t| PathBuf::from(t.trim()))),
    };
    let ctx = Ctx { out: repo.join("fit").join("out"), repo, game };
    let Some(cmd) = (!args.is_empty()).then(|| args.remove(0)) else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    if cmd == "body" {
        // The SDK's C API reads NM's build back from a dump, which carries no links since SDK v0.5: they go in as the
        // Playroom takes them (`NM_LINKS`), DMS's own derived ones, before anything reads them (a OnceLock). They're
        // derived in a process of their own (`links`), as deriving them loads the SDK.
        let file = ctx.out.join("homer.links");
        if !file.is_file() {
            let mut c = std::process::Command::new(std::env::current_exe().unwrap_or_else(|_| "dmr-check".into()));
            if let Some(g) = &ctx.game {
                c.arg("--game").arg(g);
            }
            if !c.arg("links").status().is_ok_and(|s| s.success()) {
                eprintln!("dmr-check: body: NM's links couldn't be derived (dmr-check links)");
                std::process::exit(1);
            }
        }
        // SAFETY: at the start of main, before any other thread is made.
        unsafe { std::env::set_var("NM_LINKS", &file) };
    }
    let r = match cmd.as_str() {
        "fit" => fit(&ctx, &args),
        "body" => body_check(&ctx, &args),
        "links" => links(&ctx),
        _ => Err(USAGE.into()),
    };
    if let Err(e) = r {
        eprintln!("dmr-check: {e}");
        std::process::exit(1);
    }
}

/// NM's data read from the game as the module reads it, once per process: fred.xrt into the SDK, and Homer's build
/// (the fit's template) with its dump.
fn template(ctx: &Ctx) -> Result<&'static (delirium::ragdoll::Build, Vec<delirium::prelude::Transform>, String), String> {
    static TEMPLATE: std::sync::OnceLock<Result<(delirium::ragdoll::Build, Vec<delirium::prelude::Transform>, String), String>> = std::sync::OnceLock::new();
    let game = ctx.game()?.to_path_buf();
    TEMPLATE
        .get_or_init(|| {
            let g = dms_game::Game::open(&game)?;
            if let Some(m) = g.materials() {
                dms_game::set_surfaces(&m);
            }
            let fred = g.fred()?;
            // SAFETY: `fred` is `len` bytes.
            if unsafe { sdk::nm_init_memory(fred.as_ptr(), fred.len() as i32) } < 0 {
                return Err("nm_init_memory: fred.xrt didn't load".into());
            }
            let (b, frames) = g.homer()?;
            let dump = b.dump(&frames);
            Ok((b, frames, dump))
        })
        .as_ref()
        .map_err(Clone::clone)
}

/// `dmr-check links`: the links DMS derives from the idle clip (`dms_game::links`, the golden checkout holds them to
/// the Playroom's) -> fit/out/homer.links, for the SDK's C API (`NM_LINKS`).
fn links(ctx: &Ctx) -> Result<(), String> {
    template(ctx)?;
    let l = dms_game::links().ok_or("DMS derived no links from the idle clip")?;
    std::fs::create_dir_all(&ctx.out).map_err(|e| format!("{}: {e}", ctx.out.display()))?;
    let file = ctx.out.join("homer.links");
    write(&file, l.iter().map(|q| format!("{} {} {} {}\n", q.x, q.y, q.z, q.w)).collect::<String>())?;
    println!("dmr-check: {} links -> {}", l.len(), file.display());
    Ok(())
}

fn write(path: &Path, data: impl AsRef<[u8]>) -> Result<(), String> {
    std::fs::write(path, data).map_err(|e| format!("{}: {e}", path.display()))
}

/// The scenarios of fit/scenarios/: all, or the named ones.
fn scenarios(ctx: &Ctx, names: &[String]) -> Result<Vec<PathBuf>, String> {
    let dir = ctx.repo.join("fit").join("scenarios");
    let mut all: Vec<PathBuf> = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "txt")).collect();
    all.sort();
    if !names.is_empty() {
        all.retain(|p| names.iter().any(|n| p.file_stem().is_some_and(|s| s == n.as_str())));
        if all.len() != names.len() {
            return Err(format!("no such scenario among {}", dir.display()));
        }
    }
    Ok(all)
}

/// `dmr-check body [<mdl>]`: `body.rs`'s two steps.
fn body_check(ctx: &Ctx, args: &[String]) -> Result<(), String> {
    let model = args.first().filter(|m| !m.starts_with("--")).cloned().unwrap_or_else(|| DEFAULT_MODEL.into());
    let (b, frames, dump) = template(ctx)?;
    if body::fitted(&model, (b, frames), dump)? {
        println!("dmr-check: body {model} OK");
        Ok(())
    } else {
        Err(format!("body {model}: off"))
    }
}

/// A COM gap (m) the record's three decimals can't tell from none.
const REC_TOLERANCE: f32 = 0.0015;

/// ROADMAP M10: a fitted body against its own build in LEAD on the bench (NM's own physics, the room's reactions and
/// the scenario as the Playroom's code runs them), since the Playroom can't load a fitted build (ADR 0025); FOLLOW on
/// its native body is held to it by the scenario's tolerances. A window scenario's commands go at their times.
fn fit(ctx: &Ctx, args: &[String]) -> Result<(), String> {
    let mut names: Vec<String> = args.to_vec();
    let model = names.iter().position(|a| a == "--model").map(|i| {
        let m = names.get(i + 1).cloned().unwrap_or_default();
        names.drain(i..(i + 2).min(names.len()));
        m
    });
    let model = model.unwrap_or_else(|| DEFAULT_MODEL.into());
    if names.is_empty() {
        names = ["stand", "push", "shot", "catchfall"].map(String::from).to_vec();
    }
    let bench = dmr_ctl::bench()?;
    let mut channel = dmr_ctl::Channel::bench(&bench)?;
    let short = Path::new(&model).file_stem().unwrap_or_default().to_string_lossy().into_owned();
    let out = ctx.out.join("fit").join(&short);
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let mut failed = vec![];
    for path in scenarios(ctx, &names)? {
        let mut s = dmr_ctl::Scenario::read(&path)?;
        s.mode = format!("{} fit={model}", s.mode);
        let mut recs = vec![];
        for lead in [true, false] {
            println!("dmr-check: {} on {short}: {} (about {:.0} s)...", s.name, if lead { "LEAD (the reference)" } else { "FOLLOW on its native body" }, s.seconds());
            match dmr_ctl::run(&mut channel, &s, None, lead) {
                Ok(r) => {
                    write(&out.join(format!("{}.{}.rec", s.name, if lead { "lead" } else { "follow" })), &r)?;
                    recs.push(r);
                }
                Err(e) => {
                    println!("dmr-check: {}: FAILED on the bench: {e}", s.name);
                    break;
                }
            }
        }
        let [lead, follow] = &recs[..] else {
            failed.push(s.name);
            continue;
        };
        let (theirs, ours) = (dmr_ctl::rec::parse(lead), dmr_ctl::rec::parse(follow));
        let d = dmr_ctl::rec::compare(&ours, &theirs, REC_TOLERANCE);
        let checks = tolerances(&s.tol, &ours, &theirs);
        for (what, ok, detail) in &checks {
            println!("dmr-check: {}: {} {what}: {detail}", s.name, if *ok { "ok  " } else { "OFF " });
        }
        println!("dmr-check: {}: {} steps (LEAD {}); COM gap max {:.4} m at step {}, mean {:.4} m; |v| gap max {:.3} m/s",
            s.name, d.steps, d.theirs_steps, d.com_max.0, d.com_max.1 + 1, d.com_mean, d.speed_max);
        for l in follow.lines().chain(lead.lines()).filter(|l| l.starts_with("# dmr:")) {
            println!("dmr-check: {}: {l}", s.name);
        }
        if d.steps == d.theirs_steps && !checks.is_empty() && checks.iter().all(|c| c.1) {
            println!("dmr-check: {}: OK, FOLLOW on {short} within the scenario's tolerances of its LEAD", s.name);
        } else {
            failed.push(s.name);
        }
    }
    if failed.is_empty() { Ok(()) } else { Err(format!("{short} in FOLLOW differs from its LEAD: {}", failed.join(", "))) }
}

/// The checks of a scenario's `#! tol` line on the FOLLOW record against the LEAD one: `(check, held, what was
/// measured)`. Box3D isn't NM's own physics and can't keep its record to the millimetre: the checks hold what the
/// behaviour is (fit/README.md).
///   steps=N        the COM checks over the first N steps only
///   com_max=M      the COM never further than M m from LEAD's
///   com_mean=M     on average no further than M m
///   end_z=M        the COM's height at the end within M m of LEAD's
///   fall           NM's balancer gives up (`FB balance slot 8`) in both or in neither, within 0.25 s
///   down=Z:S       the COM first drops below Z m within S s of when LEAD's does (or in neither)
///   marks          the commands at the same steps (each mark's first two words)
fn tolerances(tol: &[String], ours: &dmr_ctl::rec::Rec, theirs: &dmr_ctl::rec::Rec) -> Vec<(String, bool, String)> {
    let steps = tol.iter().find_map(|t| t.strip_prefix("steps=")?.parse::<usize>().ok()).unwrap_or(usize::MAX);
    let n = ours.steps.len().min(theirs.steps.len()).min(steps);
    let gaps: Vec<f32> = ours.steps.iter().zip(&theirs.steps).take(n)
        .map(|(a, b)| ((a.com[0] - b.com[0]).powi(2) + (a.com[1] - b.com[1]).powi(2) + (a.com[2] - b.com[2]).powi(2)).sqrt())
        .collect();
    let max = gaps.iter().copied().fold(0.0f32, f32::max);
    let mean = if n > 0 { gaps.iter().sum::<f32>() / n as f32 } else { 0.0 };
    let fall = |r: &dmr_ctl::rec::Rec| r.log.iter().find(|(_, l)| l.contains("FB balance slot 8")).map(|(t, _)| *t);
    let down = |r: &dmr_ctl::rec::Rec, z: f32| r.steps.iter().find(|s| s.com[2] < z).map(|s| s.t);
    let short = |m: &[(f32, String)]| m.iter().map(|(t, l)| format!("{t:.2} {}", l.split_whitespace().take(3).collect::<Vec<_>>().join(" "))).collect::<Vec<_>>();
    let when = |t: Option<f32>| t.map_or("never".to_string(), |t| format!("{t:.2} s"));
    let mut out = vec![];
    for t in tol {
        let (k, v) = t.split_once('=').unwrap_or((t.as_str(), ""));
        let num = v.parse::<f32>().unwrap_or(f32::NAN);
        match k {
            "steps" => {}
            "com_max" => out.push((t.clone(), max <= num, format!("{max:.3} m over {n} steps"))),
            "com_mean" => out.push((t.clone(), mean <= num, format!("{mean:.3} m over {n} steps"))),
            "end_z" => {
                let (a, b) = (ours.steps.last().map_or(f32::NAN, |s| s.com[2]), theirs.steps.last().map_or(f32::NAN, |s| s.com[2]));
                out.push((t.clone(), (a - b).abs() <= num, format!("{a:.3} m (LEAD {b:.3})")));
            }
            "fall" => {
                let (a, b) = (fall(ours), fall(theirs));
                let ok = match (a, b) { (Some(a), Some(b)) => (a - b).abs() <= 0.25, (None, None) => true, _ => false };
                out.push((t.clone(), ok, format!("{} (LEAD {})", when(a), when(b))));
            }
            "down" => {
                let (z, s) = v.split_once(':').and_then(|(z, s)| Some((z.parse::<f32>().ok()?, s.parse::<f32>().ok()?))).unwrap_or((f32::NAN, f32::NAN));
                let (a, b) = (down(ours, z), down(theirs, z));
                let ok = match (a, b) { (Some(a), Some(b)) => (a - b).abs() <= s, (None, None) => true, _ => false };
                out.push((t.clone(), ok, format!("below {z} m at {} (LEAD {})", when(a), when(b))));
            }
            "said" => {
                // `said=TEXT:S`: the log's first line starting with TEXT (`_` for a space) within S s of the
                // Playroom's (or in neither): the room layer reacted as the Playroom's did.
                let (text, s) = v.rsplit_once(':').map(|(t, s)| (t.replace('_', " "), s.parse::<f32>().unwrap_or(f32::NAN))).unwrap_or_default();
                let said = |r: &dmr_ctl::rec::Rec| r.log.iter().find(|(_, l)| l.starts_with(&text)).map(|(t, _)| *t);
                let (a, b) = (said(ours), said(theirs));
                let ok = match (a, b) { (Some(a), Some(b)) => (a - b).abs() <= s, (None, None) => true, _ => false };
                out.push((t.clone(), ok, format!("\"{text}\" at {} (LEAD {})", when(a), when(b))));
            }
            "marks" => {
                let (a, b) = (short(&dmr_ctl::rec::marks(ours)), short(&dmr_ctl::rec::marks(theirs)));
                out.push((t.clone(), a == b, if a == b { format!("{} commands", a.len()) } else { format!("FOLLOW {} | LEAD {}", a.join(", "), b.join(", ")) }));
            }
            _ => out.push((t.clone(), false, "unknown check".into())),
        }
    }
    out
}
