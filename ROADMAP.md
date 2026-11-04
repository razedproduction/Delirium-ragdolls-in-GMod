# DMR (Delirium Ragdolls in GMod) — Roadmap

Delirium (an active ragdoll, NM-style) on Garry's Mod's own ragdolls, NPCs and Workshop models in **FOLLOW** mode: NM's
own body (Homer's build from the player's GTA IV) fitted to the model's skeleton, made natively by the VPhysics-Box3D
fork under the model, driven by Delirium every physics step, the model posed from it. Split off the golden project
(Delirium-physics-in-GMod, Homer held to the Playroom one to one) on 2026-11-04: [ADR 0036](docs/adr/0036-split-from-golden.md).

## Inputs

| Component | Where | What it gives | Rule |
|---|---|---|---|
| Delirium SDK **v0.6.0** | `../delirium-physics (CLAUDE DONT CHANGE ANYTHING HERE)/crates/*` | the crates `delirium`, `delirium-data`, `delirium_physics` (C API `nm_*`) | **read only**, path dependencies (ADR 0001) |
| The golden checkout | `../DMS in Gmod`: `tools/dms-game`, `tools/dms-vphysics`, `tools/dms-ctl`, `vphysics/` | NM's data from the game, the fork's `IPhysicsNM` (`VBOX3D_NM_006`), the bind and native bodies, the hulls, the bench's channel; the fork's build | **read only**, path dependencies (ADR 0036); fixes go there, held to the Playroom |
| The player's GTA IV | `dmr_game_dir` / `data/dmr/game_dir.txt` | fred.xrt, Homer's build (the fit's template), taskparams, materials, weapons | never copied, never committed (ADR 0006) |

## The split (S0)

- [x] The repository made by the user as a copy of Delirium-physics-in-GMod's `main` with its history; the SDK v0.6.0
      update (the golden side's `71bc22b`) brought in.
- [x] Homer out: `tools/dms-homer`, `homer.lua`, his face and overlay, `BindHomer`/`HomerBody`/`Follow`/`BenchFollow`,
      the tool's throw and pins, `tools/dms-map`, Homer's golden scenarios and checks, `docs/homer.md`.
- [x] Names of its own: `gmsv_dmr_win64.dll`, `DMR`, `dmr.*`, `dmr_*`, `addons/dmr`, `data/dmr`, the bench's port 27098.
- [x] The core from the golden checkout by path (`dms-game`, `dms-vphysics`, `dms-ctl`, the fork); `tools/dmr-ctl`
      (this checkout's `.bench`) and `tools/dmr-check` (`fit`, `body`) in place of `dms-ctl`'s binary and `golden`.
- [x] Checked on the bench (2026-11-04, beside the golden addon, both modules loaded; built against the golden side's
      PR 39): `dmr-check body` on male_07 OK (the build within 1.2e-7 of the SDK's `nm_build_fit` with DMS's links,
      the native body's frames within 2.2 µm, masses, damping and tensors within 6.4e-7 — the head's 32 % of the
      golden side's v0.6.0 notes is gone —, dropped without NM: joints together, within NM's limits); `dmr_ragdoll`
      stands a citizen under `bodyBalance`. `dmr-check fit` as on the golden side after SDK v0.6.0 (not the split's):
      `stand` within; `push` COM mean 0.056 m (tolerance 0.05), `shot` 0.286 (0.25; 0.255 there), `catchfall` the end
      0.218 m against LEAD's 0.176 (0.03; 0.220 / 0.165 there).
- [ ] The Workshop item of the DMR addon (published by the user from `addon\`); its id into `scripts\release\install.bat`
      (`WORKSHOP_ID`) and `docs/install.md`.
- [ ] Release 0.1.0 (`DMR-0.1.0-win64.zip`, `docs/release-notes-0.1.0.md`): waits for the Workshop item and the user.

## Next

- [ ] Both projects driving bodies at once: the fork keeps one step callback per environment, so the module that last
      puts NM on a body takes it from the other. A fork change (several callbacks, a new `VBOX3D_NM_*`) in the golden
      repository, or one host for both.
- [ ] The module's FOLLOW core from the golden side's library (its ADR 0037, `crates/dms-core`) instead of this copy of
      `module/`, once it's merged there: the shared FOLLOW code then reaches this project without porting by hand.
- [ ] M10's `push` gap on fitted bodies (COM mean 0.062 m against 0.05): the stance divergence of M4/M5, the fork's
      joints — the golden side's to fix.
- [ ] M12's DoD: 10 Workshop player models with other rigs (none mounted yet), M12.5's DoD with one that needs a rig.

## History: GMod's ragdolls, the Workshop's included (M9–M12.5, before the split)

The goal: any humanoid ragdoll of the game — HL2's citizens, combine, players' models, NPCs' death ragdolls and models
from the Workshop — gets Delirium the way Homer has it, keeping its own look. The design is proposed in
[ADR 0025](docs/adr/0025-gmod-ragdolls-nm-proxy.md) (accepted after M9's spikes; the fork's part ADR 0026). The previous attempt's
`delirium-ragdoll` stays ignored: this is the new tool and the new approach.

**The approach: an NM body fitted to the model, the model posed from it.** It's how Delirium's own SDK drives other
skeletons (the Unity bridge `NmCharacter`, `fitToSkeleton`): `nm_build_fit` (`Build::fit`) fits Homer's build to the
skeleton's 21 joints (bone lengths, masses by the cube of the scale), the host builds the 21 parts as its physics, NM
drives them in FOLLOW, and the model's bones are posed from the parts. Everything M3–M8 built for Homer — FOLLOW on
Box3D, the bind, the room layer, the golden comparison — works unchanged on such a body, because it *is* an NM body:
only its sizes differ. The model's own `.phy` (15 objects on ValveBiped, arbitrary on the Workshop) isn't used for the
physics at all, so its quality doesn't matter.

What changes against Homer and needs care:
- **The behaviours are tuned for ~1.8 m** (Delirium's docs): a fitted body far from it behaves worse. ValveBiped is
  ~1.83 m, so HL2's models are close; the Workshop's extremes get a warning.
- **The golden comparison** stays possible: the Playroom takes a build dump (`delirium-room <xrt> <build.txt>`), so the
  fitted build can be run there too. Whether the links Delirium derives for Homer (`nmdbm::DERIVED_LINKS`) apply is a
  spike question.
- **The look is the model's**, not the capsules': a model's mesh doesn't match NM's capsules exactly (clipping a bit
  into the floor or itself is possible, as anywhere an active ragdoll's body is simpler than its mesh).

### M9. Spikes and the decision — 3–5 days

Each spike ends with numbers and a recommendation; ADR 0025 is accepted (or replaced) at the end.

- [x] **S1, the skeleton:** read any model's bones, hierarchy and bind pose — from the `.mdl` bytes (`file.Read(model,
      "GAME")`, mounted Workshop GMAs included) in the module, or through the game (`SetupBones` on the reference pose).
      Pick the more reliable one on HL2's models and 5–10 Workshop models.
- [x] **S2, the fit:** the ValveBiped → NM joint map (as `NmCharacter.Joints`: NM's 4 spine joints spread from Spine to
      Neck, a missing neck/clavicle put between its neighbours), `Build::fit` on male_07, combine, alyx, a tall and a
      short Workshop model: sizes, masses, how far from Homer; the fitted build in the Playroom (`<xrt> <build.txt>`):
      does it stand, does it need Homer's links.
- [x] **S3, the physics body** — the main choice:
  - **(a) native bodies:** a new `IPhysicsNM` (`VBOX3D_NM_005`) creates the 21 NM bodies and joints straight from the
    fitted build (capsules/boxes, masses, tensors, pivots, limits) in the ragdoll's environment; the game's
    `prop_ragdoll` of the model stays the networked, traced, physgun-able entity, its objects following the NM bodies;
    clients need nothing but the model they already have;
  - **(b) a generated proxy model:** `dms-ragdoll` (new) compiles an invisible NM-body ragdoll per model (as `dms-homer`
    does Homer, with `studiomdl`), cached in `garrysmod/models/dms/proxy/`; the proxy is the physics, the model is drawn
    posed from it; remote clients would need the proxy model too.
  Prototype both on male_07; compare: binding, FOLLOW fidelity (`golden body`-style numbers), physgun/gravity gun,
  bullets and hit groups, networking to a second client, first-use delay.
- [x] **S4, posing the model:** the model's mapped bones from the parts (each with its bind-pose offset to its part),
      unmapped bones (fingers, toes, jiggle, accessories) following their parents; where it happens (the server's
      ragdoll objects, or the client's `BuildBonePositions`); skins, bodygroups and flexes kept.

**DoD:** ADR 0025 accepted with the chosen S1/S3/S4 variants and their numbers; male_07 stands under `bodyBalance` in a
prototype.

**Done (ADR 0025, 0026):** S1 the `.mdl` read in the module (97 models, equal to the engine's bind pose); S2 NM's spine
joints spread in Homer's proportions (male_07 1.74 m, 81 kg; spread evenly: 130 kg), a fitted body's golden on the crate
(the Playroom's `<build.txt>` can't carry one); S3 **(a) native bodies**, `VBOX3D_NM_005` (the proxy (b) prototyped too:
the same fidelity, a 7.3 s compile, clients lack it); S4 posed on the server (15 objects without the model's own
constraints, within 7 mm), the 6 bones without objects finished on the client. Prototypes: `dms_spike_native`,
`dms_spike_proxy` (`addon/lua/dms/spike_ragdolls.lua`).

### M10. ValveBiped ragdolls — 5–8 days

- [x] The chosen skeleton reader, the ValveBiped map and `Build::fit`; fitted builds cached by model and its CRC.
- [x] The NM body for a model (S3's variant), the bind (M3's `bind.rs`, generalised from Homer to any fitted body),
      FOLLOW (M4) unchanged.
- [x] The swap: `dms_attach` on any ValveBiped `prop_ragdoll` — the NM body takes the ragdoll's current pose and every
      part's velocity, so a falling ragdoll keeps falling; `dms_detach` gives the model its own physics back the same
      way.
- [x] The model's look kept: posing (S4), skin, bodygroups, colour, material.
- [ ] Golden: the fitted build of male_07 against GMod (`stand`, `push`, `shot`, `catchfall`), the tolerances as for
      Homer — its reference its own LEAD on the crate (`golden fit`, there), the Playroom can't load a fitted build (M9).
      `stand` and `catchfall` within them; `push` (COM mean 0.061 m, tolerance 0.05) and `shot` (0.378 m, 0.25) off:
      it steps away further than its LEAD (Homer: 0.020, 0.148), never falls, ends as high.
      → The bench, 2026-11-03 (`golden fit`, the fork `VBOX3D_NM_006`): `stand`, `catchfall`, `shot` (COM mean 0.213 m,
      tolerance 0.25), `stand-1s` (COM max 0.060 m, 0.08) within them; only `push` off: COM mean 0.0615–0.0621 m
      over 3 runs (tolerance 0.05), max 0.079 (0.10), no fall, the end height 0.956 m against 0.963. The gap is made
      **before** the poke (at 2.00 s): 0.057 m at 1.00 s, the poke adds ~0 (LEAD's COM goes 2.5 cm in the 6 s after
      it, FOLLOW's 0.3 cm). It's Homer's stance divergence (ADR 0019), on a body that settles further: in the first
      0.25 s FOLLOW's COM dips to 0.943 m against LEAD's 0.964 and takes its first step elsewhere (x +0.029 against
      −0.013 at 0.75 s); standing, the COM is 6–7 mm lower (0.949 against 0.955), the feet's deepest contact
      −12…−14 mm against NM's −7 (tilted feet), the joints' pivots apart up to 6 mm; and NM's own physics creeps on
      the floor (LEAD ~2 mm/s while "still", then steps at ~30 s of `stand`, 0.58 m away at its end, as the
      Playroom's Homer does at ~40 s; Box3D's friction holds the feet). Homer's
      `push` has the same parts, smaller: 0.027 m at 1.00 s. What's left is M4/M5's stance item (the fork's joints).
- [x] The body check (`golden body`) on the fitted body (moved to M11; done there: `golden body --fit`).
- [x] The DMS window: "NM on the ragdoll under the crosshair" works on any model, the model's fit shown (height, mass,
      warnings).

**Done (ADR 0027):** the fork's `VBOX3D_NM_006` (NM's self-collision pairs, `SetBodyState`, kept hulls: a model's second
body in 0.9 ms against 0.9 s); the swap both ways (a falling ragdoll keeps falling on NM, and its own physics comes back
moving as NM's body was, as a new ragdoll); the model's joints put in Homer's stance before the fit (ValveBiped's
reference pose stood the body with its arms 40° out and its neck forward); the 6 bones without objects finished on the
client in `RenderOverride` (within 1.4 mm of NM's parts; `BuildBonePositions` never runs for a server ragdoll); the
physgun and gravity gun grab NM's body (the Playroom's grab and poke), bullets hit their physics bone's part; get-up by
animation off on fitted bodies (they stay down). The golden of a fitted body is its build in LEAD on the bench
(`golden fit`): the Playroom can't load a fitted build. HL2's citizens, combine, police and alyx stand: 1.81 m / 81 kg,
the women 1.72 m / 69 kg.

**DoD:** HL2's citizens (male/female), combine, police, alyx stand, react and fall as Homer does; the golden scenarios on
male_07's fitted build within tolerance; the look of the model unchanged.

### M11. Death ragdolls and auto-attach — 3–5 days

- [x] NPCs: their server ragdolls (`CreateEntityRagdoll`, `ai_serverragdolls`) get NM at once, in the dying pose and
      velocity; the reaction to the killing hit as the Playroom's death and shot layer (ADR 0016).
- [x] Players: GMod's player death ragdolls are client-side (they can't have FOLLOW, ADR 0002) — a server ragdoll of
      the player's model instead (the client one suppressed), the camera on it as GMod's death camera.
- [x] Convars: `dms_autoattach_npc`, `dms_autoattach_players`, a limit (M7's LOD and budget); removal with the ragdoll's
      fade/cleanup.
- [x] M10's ends: `golden body` on a fitted body (`golden body --fit`); the duplicator and saves of a ragdoll with NM
      (the copy got the original's fields and stayed frozen).

**DoD:** killing NPCs and players gives NM ragdolls reacting to the last hit as in the Playroom, without the frame time
going over M7's budget at the limit.

**Done (ADR 0028):** a ValveBiped NPC's lethal damage makes its ragdoll on the server without the game's push, and a tick
later NM is on it (undo and cleanup take it in the NPC's place); a dead player's client ragdoll gives way in the same
tick to a server ragdoll of their model posed from their bones, the death camera on it as GMod's. The killing hit is the
room's on a fatally injured body (health 49, then the hit's reaction): the ShotTask's dying reaction, no getting up, later
hits reacting till health 0, the room's death (`STOP all`, `relax`). (`fatal=1`, the golden `death`, left a corpse only
limp: the user chose fatally injured.) The motion is handed over (the fork's pending velocity made the body's own first: with the game's
push left on NM's body took 44.6 m/s). `dms_autoattach_max` 4 (within `dms_max_agents`) and `dms_autoattach_time` 30 s;
NM off is the swap. A model's hulls are made ahead as NPCs and players spawn (a death ragdoll in 1.1 ms, not 914 ms).
On the bench: 8 NPCs shot 0.3 s apart, limit 4: the tick 14.8 ms mean, 21 ms worst, NM 3.2 ms a step for the 4 (M7's
1–2 ms an agent). Copies of a ragdoll with NM get NM again. `golden body --fit` on male_07, female_01, combine_soldier:
the build within 1e-6 of the SDK's `nm_build_fit`, the native body's frames within 2.2 µm, masses and tensors exact.

### M11.5. Live bodies: NPCs' reactions to shots, the drunk player — 2–4 days

One mechanism for both: the living entity → its model's server ragdoll with NM (posed from its bones, as M11's
player corpse) → back on its feet where the body is.

- [x] NPCs: a ValveBiped NPC shot and not killed becomes its ragdoll with NM, the shot the Playroom room's (`shot at=
      dir= weapon= part=`, no `fatal`); the NPC hidden meanwhile (health, relationships, weapon, AI kept); hits on the
      ragdoll are its own (the gamemode's hitgroup scaling); back standing in the ragdoll's place once the ShotTask is
      over and the body has settled; killed meanwhile: M11's death on that ragdoll (fatally injured).
- [x] Players: `dms_drunk` (and a button in the DMS window) makes the player their ragdoll with the Playroom's
      `drunk` button, the camera on it as M11's death camera; `dms_drunk` again stands them up there.
- [x] Convars `dms_live_npc`, `dms_live_max` (within `dms_max_agents`), `dms_live_time`; `DMS.FollowRoom` tells
      whether the ShotTask and the explosion task run.
- [x] Golden `drunk` on Homer (the Playroom's button from the stance).

**DoD:** on the bench a shot at a live NPC gives the Playroom's reaction and the NPC comes back alive; the drunk mode
turns on and off with the camera on the body; M7's and M11's limits hold; Homer's golden unchanged.

**Done (ADR 0029):** a citizen shot (10 of 100) became its ragdoll with NM in 32 ms (NM's world crop ~28 ms, as an M11
death ragdoll's), reacted with the room's ShotTask (staggered, fell), and ~10 s later, the task over, stood up where the
body lay with 90 health and its SMG, walking on command; killed as a ragdoll it stayed that ragdoll as a fatally injured
corpse (`health 49`, `shotRelax`), the game's death ragdoll gone, its weapon dropped there. 6 citizens shot 0.3 s apart:
4 ragdolls (`dms_live_max`), the tick 15.0 ms mean, 63.7 ms worst, NM 2.5 ms a step. `dms_drunk` on and off (the player
hidden, the camera's entity on the body, then standing and walking); dying drunk, the ragdoll is the corpse. From the
player's pose the drunk body fell within ~2 s in 2 runs of 3 (Homer doesn't in the Playroom, nor a fitted body from NM's
stance in `golden fit drunk`); not tuned (ADR 0008). Golden `drunk` on Homer: no fall, the end height and the commands'
steps as the Playroom's, the wander's COM apart (1.86 m).

### M12. Workshop models and other humanoids — 5–8 days

- [x] Skeletons beyond ValveBiped: classification (ValveBiped, known rigs by bone names, unknown); unknown humanoids
      through a rig of their own: a bone per NM joint (and the joints, sizes, masses, placement), saved in
      `data/dms/skeletons/<model>.json`, text to share; the addon reloads it as it changes. (The **editor** with the
      capsules shown live over the model is its own application: M12.5. An in-game tab with sliders was tried and
      removed, the user's choice.)
- [x] Models from the Workshop: read from the mounted GMAs (nothing of them copied or redistributed); first use of a
      model: the fit in the background with a message, then cached.
- [x] Limits stated: non-humanoids (animals, robots without a humanoid chain) aren't supported (a message); far from
      1.8 m — a warning (Delirium's tuning); models with broken bind poses — reported by the reader.
- [x] Multiplayer: what a joining client needs (with S3 (a): only the model, which GMod already sends or the client
      subscribes to).

**DoD:** 10 popular Workshop player models (different rigs) work — automatically or after a mapping saved once — and a
mapping made on one machine imports on another.

**So far (ADR 0030, proposed):** a skeleton's class by its rig (ValveBiped, Biped, Mixamo, Unreal, TF2, VRoid, MMD), a
guess by its bones' names and hierarchy, or the model's rig file (`data/dms/skeletons`, format 2: bones, joints moved,
parts' sizes, masses and placement; reloaded as the file changes, the model's ragdolls with NM refitted), checked
before it's fitted; physics bones that are no joint carried by their nearest posed ancestor's part; the client's
finishing per model; a broken bind pose fitted to the mesh's bind and reported; a model's first use prepared in the
background with a message; NM's bodies drawn over the ragdolls (`dms_rig_view`). The fit is the crate `tools/dms-fit`,
shared with M12.5's editor. On the bench: HL2's models and the mounted Workshop ValveBiped ones (Terminator's arnold
and t800nw, EFT's bear and usec) map as ValveBiped and `golden body --fit` holds (within 1e-5 of the SDK's
`nm_build_fit`); `zombie/classic` (no Neck1) maps by names; antlion, dog, headcrab and vortigaunt are refused as not
humanoid; five models stand under `bodyBalance`; a rig file written beside the running game refits its ragdoll (and
removed, gives it back its own). Homer's golden: `stand`, `push`, `shot`, `catchfall`, `blast`, `getup`, `death` within
the tolerances; `drunk` falls in 2 runs of 3 on this build and on `origin/main` alike (not M12's). Open: the 10 Workshop
player models with other rigs (none mounted yet; with M12.5's editor for the ones that need a rig).

### M12.5. The rig editor (`tools/dms-rig`) — 5–10 days

An application of its own (Rust, egui; win64, no AVX2), the user's choice: editing a rig with sliders in the game is
awkward. It writes the rig files the addon reloads (M12), so the game can stay open beside it.

- [x] Models: GMod's own and the Workshop's read where they are (the game's VPKs, loose files, the mounted GMAs: read
      only, nothing copied): `.mdl` (bones), `.vvd` and `.vtx` (the mesh), drawn in its own pose (the mesh's bind).
- [x] The rig over the model: NM's capsules and boxes from `tools/dms-fit` (the module's own fit, GTA IV's data as the
      module reads it), refitted as anything changes.
- [x] Editing by the mouse: a bone picked by clicking it, joints dragged in 3D, a capsule's radius and length by its
      handles, a part turned by rings; numbers editable beside; undo; the rig file saved (format 2), opened, shared.
- [x] **Simulation on the fork itself**, step 1: `vphysics.dll` (VPhysics-Box3D, `IPhysicsNM` `VBOX3D_NM_006`) loaded
      into the application, the fitted body made natively as the module makes it (a floor, gravity), dropped, thrown
      and dragged by the mouse without NM: the capsules, joints, limits and masses checked on the physics the game runs.
- [x] Step 2: NM in FOLLOW in the application, as in the game (`bodyBalance`, a push, a shot, NM's messages): the
      module's FOLLOW (`follow`, `bind`, `step`) out into a crate without Lua, Homer's golden held after it.

**DoD:** a Workshop model whose rig isn't found by itself is made to stand under NM in the game with a rig made in the
editor in minutes, without touching a slider in the game; the rig file opens on another machine the same.

**So far (ADR 0031):** `tools/dms-rig` (eframe/egui, OpenGL) reads the game's VPKs, loose files and the Workshop's GMAs
(13 697 models here), draws a model's mesh in its own pose with NM's parts fitted over it by `dms_fit` (male_07: 1.81 m,
81.2 kg, as the module), edits the rig by the mouse (joints, a capsule's radius and length, a box's extents, a part
moved and turned on its bone by handles and rings, bones picked by a click; numbers beside; undo) and saves, imports,
pastes and copies the rig file. The bind and the native body moved out of the Lua natives into `tools/dms-vphysics`
(with the `IPhysicsNM` bindings and a host for the fork outside the game); the editor loads the game's `vphysics.dll`
and drops, throws, kicks and drags the fitted body on it (a concrete floor with NM's row, 127 surface properties, 0.4 ms
a step), the model's mesh skinned from the parts. On the bench after the move: Homer's `stand`, `push`, `shot`,
`catchfall`, `blast`, `getup`, `death` within the tolerances (`push` COM mean 0.020 m as before), `golden body --fit`
on male_07 OK; a rig file written beside the running game (a thigh's radius, the head's mass and turn, a knee moved)
refitted the model's ragdoll with NM within a second, and it stood (pelvis 0.98 m after 12 s). Open: no mounted
Workshop model needs a rig of its own (62 ValveBiped, the rest not humanoid; `dms-rig --classify workshop`), and the
game's own unmapped humanoids lack bones for required parts (the stalker's and the skeleton's hands, the poison
zombie's head), so the DoD's model waits for one the user subscribes to.

**Step 2 (ADR 0032, 0033):** the FOLLOW step's pieces (`dms_vphysics::follow`) and the room's shot
(`dms_vphysics::follow::shot`: damage, injury, impulse, ShotTask) shared by the module and the editor. In the Simulate
tab: *NM: bodyBalance* on the fitted body (or Homer's), *Push* (the Playroom's poke), the grab by the mouse, *Shoot*
(the room's shot with the Playroom's weapon, from the front or behind, into any part; health and death), the
Playroom's message-only buttons and a line for any of NM's messages. `--snap-report --sim 600 --nm balance --push 40`
or `--shoot pistol` checks each body against LEAD with the same blow: Homer stands both (shot: staggers 1.8 m, health
175, the pelvis at 0.887 m as in LEAD). Golden after the step's move (PR #30): 12 of 14 room scenarios within the
tolerances, `drunk` and `relax-1s` out as on the base; the shot's move (0033) is to be checked on the bench with
`shot`, `shotgun` and `death`.

