# 0036. This repository: GMod's ragdolls with NM, on the golden project's core

Status: accepted (2026-11-04). The other side of the golden repository's 0036 (the project split). Moves in: 0025–0032
(their work goes on here).

## Context

Until 0.2.0, `razedproduction/Delirium-physics-in-GMod` held two projects: the Playroom in Garry's Mod (Homer's body,
held to the Playroom one to one, ADR 0008) and NM on GMod's own ragdolls (M9–M12.5: Homer's build fitted to any
humanoid model, native bodies, ValveBiped and Workshop skeletons, NPCs' death ragdolls, live NPCs and the drunk
player, the rig editor). The second has no reference to be held to and keeps growing behaviour of its own, and every
change to it had to be shown not to touch Homer. The golden repository's ADR 0036 splits them: it keeps Homer only,
and this repository, made as a copy of it with its history, takes GMod's ragdolls.

## Decision

1. **No Homer here.** Removed: `tools/dms-homer` (Homer's model), `addon/lua/dms/homer.lua` (his spawn, bind and
   report), his face (`cl_face.lua`) and the skeleton and debug overlay drawn on his model's bones (`overlay.lua`,
   `cl_overlay.lua`), `DMS.BindHomer`, `DMS.HomerBody`, `DMS.Follow` and `DMS.BenchFollow`, the tool's throw and pins,
   `tools/dms-map` (the Playroom's stages map), `golden body` for Homer, `golden dump/links/lead/run/behaviour` and the
   Playroom's scenarios but the four the fit is checked on (`fit/scenarios`). A bench scenario runs on a fitted body
   only (`fit=` on its mode line).
2. **"No Homer" isn't "no NM data".** The fit's template stays NM's own: Homer's *build* (`m_y_pmanhat_01`), read from
   the player's GTA IV by the core (`dms_game::Game::homer`), fitted to a model's skeleton. NM's self-test and the
   bench's LEAD agent run on it too.
3. **Names of its own, so both install side by side** on the same fork `vphysics.dll`: the module `gmsv_dmr_win64.dll`
   (`require("dmr")`, the global `DMR`), the addon `garrysmod/addons/dmr` (`autorun/*/dmr_*.lua`, `lua/dmr/`), the
   public API `dmr.*`, convars and commands `dmr_*`, hooks and net messages `DMR_*`/`dmr_*`, entity fields `DMR_*`,
   `data/dmr/` (game folder, skeletons, profiles, the bench's settings), the bench's channel on port 27098 (the golden
   one's is 27099) and `.bench/` of this checkout. The rig file's `dms_skeleton` key stays (players' rigs read as
   they are). Its own Workshop item, not the Playroom's 3829811829; its own versions from 0.1.0 (`DMR-<version>-win64`).
   DMR leaves `models/dms/homer.mdl` to the Playroom's addon.
4. **The core is the golden checkout's, read only**, as the SDK is (ADR 0001): `"../DMS in Gmod"` next to this one,
   path dependencies, never a copy. `tools/dms-game` (NM's data from the game), `tools/dms-vphysics` (the fork's
   `IPhysicsNM`, the bind, native bodies and their hulls, the host and FOLLOW pieces without Lua), `tools/dms-ctl` (the
   bench's channel, RCON, scenarios and records: `tools/dmr-ctl` wraps it with this checkout's `.bench`) and the fork
   (`vphysics/`, built there by its `scripts\build-vphysics.ps1`; `install.ps1 -Fork` and `package.ps1` take its build).
   Fixes to any of it are made there, held to the Playroom, and reach this project from there.
5. Kept here: `module/` (its copy from the split, renamed: until the golden side's FOLLOW core is a library, ADR 0037
   there, changes to the shared FOLLOW code are made there and ported here by hand), `tools/dms-fit`, `tools/dms-rig`,
   the ragdolls', deaths', live bodies' and skeletons' Lua, and `tools/dmr-check` (the golden tool's `fit` and
   `body --fit`, against this project's own references: a fitted body's FOLLOW against its build in LEAD, its native
   body against its build; the SDK takes DMS's derived links through `NM_LINKS` as there).

## Consequences

- The golden checkout must sit next to this one at a state with `dms_vphysics::hull` (its PR 39 on): it's what this
  repository builds against, whichever branch is checked out there.
- Both modules can be loaded at once, but the fork keeps one step callback per environment: the module that last puts
  NM on a body takes it, and the other's agents stop being stepped. Driving bodies with both at once needs a fork
  change (several callbacks, a new `VBOX3D_NM_*`), made in the golden repository.
- ADRs 0001–0035 stay as this project's history; the ones about Homer's body (0004, 0008's golden runs, 0013) are the
  golden project's now.
