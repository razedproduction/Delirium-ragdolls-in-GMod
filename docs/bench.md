# The bench: GMod + `dmr-ctl`

Claude starts Garry's Mod on its own, drives it and sees its state in real time; what's left to the user is the
visual check. The decisions: [ADR 0012](adr/0012-bench-game-and-dms-ctl.md) (from before the split); the channel, RCON,
scenarios and records are the golden checkout's `dms-ctl` library ([ADR 0036](adr/0036-split-from-golden.md)),
`tools/dmr-ctl` is this checkout's command line over it.

## In one command

```powershell
.\scripts\build.ps1                     # the module -> dist\
cargo build --release -p dmr-ctl -p dmr-check
.\scripts\server.ps1 start              # the game in a window, the dmr-ctl channel and RCON on 127.0.0.1
.\target\release\dmr-ctl.exe exec status
.\target\release\dmr-ctl.exe lua "return DMR.Version"
.\target\release\dmr-ctl.exe watch --for 2s
.\target\release\dmr-ctl.exe run fit\scenarios\stand.txt --fit models/humans/group01/male_07.mdl
.\target\release\dmr-check.exe body
.\scripts\server.ps1 stop
```

## `scripts\server.ps1`

- `start` installs `dist\` and `addon\` into the game (`install.ps1`; `-NoInstall` keeps what's installed), writes
  `garrysmod\data\dmr\bench.txt` (`dmr_game_dir` from `-GtaDir`, `DMR_GAME_DIR`, `fit\game_dir.txt` or the golden
  checkout's `golden\game_dir.txt`; `dmr_dev 1`, a new token) and starts **the game itself**:
  `gmod_win64.exe -w 1280 -h 720 -windowed -nosound -novid -usercon -condebug -ip 127.0.0.1 -port 27115 +maxplayers 2
  +sv_lan 1 +sv_cheats 1 +sv_hibernate_think 1 +rcon_password <new> +map gm_flatgrass`. The window is visible so the
  test can be watched. `start -Hidden`: without drawing (`-textmode`), the window hidden.
  It's a listen server: `maxplayers 2`, otherwise single player without RCON; `-usercon` turns RCON on. The addon reads
  `bench.txt` and deletes it at once, so a normal game doesn't open the channel. Addresses and secrets are in
  `.bench\server.txt` of this checkout (not in git), `dmr-ctl` and `dmr-check` read them. The console log is
  `garrysmod\console.log`.
- `stop`: `quit` over RCON, forced after 10 s; `restart`, `status`.
- The game doesn't start twice: while the bench runs, the user doesn't play, and the golden checkout's bench can't run
  (it's the same game); `start` checks. The golden addon may be installed beside this one: both modules load.
- No dedicated server: the fork doesn't run on GMod's x64 server build (newer vphysics interfaces).

## The module's channel (`module/src/dev.rs`, `addon/lua/dmr/dev.lua`)

Only with `dmr_dev 1` (0 by default), only on `127.0.0.1:dmr_dev_port` (27098; the golden one's is 27099), only with
`dmr_dev_token` (the first request is `auth`). The network threads touch neither Lua nor the physics (ADR 0003): they
queue requests and hand out finished frames. `Think` takes the queue (`DMR.DevThink`): console commands and Lua go
back to Lua, NM messages and scenarios to the bench. A frame: a line `<id> <kind> <length>` and `<length>` bytes of
UTF-8.

## `dmr-ctl`

| Command | What | Path |
|---|---|---|
| `status` | what answers | RCON and the channel |
| `exec <command>` | a console command with its output | RCON (only it gives output); `--module`: into `Think` without output |
| `lua <code>` / `lua -f <file>` | Lua in `Think`: what it printed and returned (an expression as `return …`) | the module; if it doesn't answer, RCON `lua_run` |
| `nm <message> [p=v …]` | an NM message to the LEAD bench agent (NM's own build on the Playroom's floor) | the module |
| `bench reset\|stop\|cancel` | a new LEAD agent / remove it / break off the scenario | the module |
| `snapshot [--fields a,b]` | the last telemetry (JSON) | the module |
| `watch --for 2s [--fields a,b] [--every N]` | a stream of telemetry, a JSON line a server frame; **always with `--for`** | the module |
| `run <scenario> --fit <mdl> [--out f.rec] [--lead]` | a `fit/scenarios/` scenario on a body fitted to the model, in FOLLOW on its native body (`--lead`: its build in LEAD), in real time → `.rec` (by default `fit/out/<model>/<name>.{follow,lead}.rec`) | the module |

PowerShell 5.1 drops `"` from external programs' arguments: in Lua use single quotes or `lua -f`.

Telemetry (top-level fields, `--fields` picks): `t`, `tick`, `frame_dt`, `hover`, `gravity` (`nm` 9.8, `source`,
`compensation`: the `dmr_gravity_mode` mode, ADR 0007), `source_gravity`, `nm_ms` (NM per physics step, ms), `agents`
(FOLLOW agents: `id`, `step`, `awake`, `asleep`, `balancing`, `fallen`, `pelvis_z`, `head_z`, `com`, `com_v`,
`compensation`, `parts`, `contacts`, `drives`, `armature`, `feedback`, `timestep`, `pred_err`, `limit_use`, `state_v`,
`script`), `bench`; with the LEAD agent also its own state.

## Scenarios on a fitted body (ROADMAP M10)

`dmr-ctl run --fit` (and `dmr-check fit`) runs a scenario on **a body fitted to a model**: Lua spawns the model's
ragdoll at the Playroom room's start (3 m higher for `!button fall`), makes its native body standing there
(`DMR.RagdollBody`), takes the model's own physics out of the way and hands the body to the module
(`DMR.BenchFitted`); `--lead` runs the same build in LEAD on the crate (NM's own physics, the reference). A scenario
without `fit=` is refused: there's no Homer here. The commands are the Playroom's (`!button`, `!shoot`, `!blast`,
`!poke`, `!set`, `!quit`, ...), as on the golden side; what the bench can't run is written as
`# dmr: not run on the bench: …`.

`DMR.FollowStats()`: NM's step cost on the main thread, threads, the callback's calls, each agent's step;
`DMR.DevPanic("native"|"step")` checks that a panic doesn't crash the game (ADR 0021). After `changelevel` the bench's
convars (`dmr_dev`, the token) are reset: turn the channel on again over RCON.

## The scene and NM's world (ADR 0018)

`gm_flatgrass`: a flat floor; the start of the Playroom's room is Box3D's floor (`DMR_FloorBelow`: the BSP plane + the
fork's 6.35 mm shell) under `info_player_start` (`dmr_bench_scene`; `DMR.BenchScene()` checks the floor is level).
NM's world comes from the current map's `.bsp` (`world.lua`, `dmr_map [reload]`); props within 3 m of an agent are NM's
obstacles. The Playroom's stages map `dms_stages` is the golden project's (its `dms-map`); stage scenarios run here on it
when it's built there.
