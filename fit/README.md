# fit: a fitted body against NM

There's no Playroom reference for a body fitted to a GMod model: the Playroom can't load a fitted build (ADR 0025).
`tools/dmr-check` holds a fitted body to NM itself instead, on the bench (the game itself, [docs/bench.md](../docs/bench.md)):

| Command | What it does | Output in `fit/out/` |
|---|---|---|
| `body [<mdl>]` | default male_07 (ROADMAP M11, ADR 0028): NM's build (Homer's, the fit's template, read from the game as the module reads it) fitted to the 21 joints the module fitted it to (`DMR.FitJoints`) against the SDK's own `nm_build_fit` on that build's dump (1e-5); then on the bench the fork's native body (no NM) made standing against the build (frames 1 mm / 0.5°, masses, damping, inertia 1 %), and dropped without NM for 4 s (finite, joints together, within NM's limits) | `homer.links` |
| `fit [--model <mdl>] [names]` | default male_07 and `stand`, `push`, `shot`, `catchfall` (ROADMAP M10): each scenario of `fit/scenarios/` on the bench twice, in LEAD on the build alone (NM's own physics: the reference) and in FOLLOW on its native body, the FOLLOW record held to the LEAD one by the scenario's `#! tol` line | `<model>/<name>.{lead,follow}.rec` |
| `links` | the links DMS derives from the idle clip (`dms_game::links`), which the SDK's C API takes through `NM_LINKS` (its dump carries none since SDK v0.5); `body` makes them when they're missing | `homer.links` |

```powershell
cargo build --release -p dmr-ctl -p dmr-check
.\scripts\server.ps1 start
.\target\release\dmr-check.exe --game "D:\Games\Grand Theft Auto IV" body
.\target\release\dmr-check.exe fit
.\scripts\server.ps1 stop
```

`--game` is remembered in `fit/game_dir.txt` (not in git), or the variable `DMR_GAME_DIR`. Everything in `fit/out/` is
made from the player's game files and is **not committed** (ADR 0006, `.gitignore`).

## Scenarios

`fit/scenarios/*.txt` are the Playroom's scenario scripts, as the golden project has them (`#! headless <steps>
[press=a,b]` or `#! window` with `<t> <command>` lines; `#! tol ...` the checks). One runs by hand with
`dmr-ctl run fit\scenarios\<name>.txt --fit <model.mdl> [--lead]`. A window scenario's commands go at their times (the
golden side puts them at the steps the Playroom's own record has them; there's none here).

The tolerances (`#! tol`), FOLLOW against LEAD:

| Check | Holds when |
|---|---|
| `steps=N` | the COM checks over the first N steps only |
| `com_max=M` | the COM never further than M m from LEAD's |
| `com_mean=M` | on average no further than M m |
| `end_z=M` | the COM's height at the end within M m of LEAD's |
| `fall` | NM's balancer gives up (`FB balance slot 8`) in both or in neither, within 0.25 s |
| `down=Z:S` | the COM first drops below Z m within S s of when LEAD's does (or in neither) |
| `said=TEXT:S` | the room's log line starting with TEXT (`_` for a space) within S s of LEAD's (or in neither) |
| `marks` | the commands at the same steps |
