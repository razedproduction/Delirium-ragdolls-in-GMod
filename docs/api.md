# DMR's Lua API

GMod's humanoid ragdolls in FOLLOW mode: NM (Delirium) drives a body fitted to the model (its native body, made by the
fork, the model posed from it: ADR 0025, 0027), and the Playroom's reaction layer (ADR 0016, 0020) answers what the
game does to it. Everything is on the server; only a superadmin can change anything from the console and the menu.

## Quick start

```lua
-- Any humanoid prop_ragdoll (a citizen: dmr_ragdoll; from the spawn menu with dmr_autoattach_ragdolls 1).
local c = dmr.Attach(rag)          -- NM on the ragdoll + bodyBalance (the Playroom's "balance" button)
c:Shot(9, nil, "shotgun")          -- a shotgun shot in the chest: applyImpulse + the Playroom's ShotTask
c:Send("headLook", { start = true, pos = ply:EyePos() })
print(c:GetState().health)         -- 200 for a living body, damage is the game's ped::damage
local _, rag2 = dmr.Detach(rag)    -- the model's own ragdoll again, moving on as NM's body was
```

## Versions

`DMR.Version` (the module), `DMR_ADDON_VERSION` (the Lua addon), `DMR.DeliriumVersion` (the Delirium built into the
module), `DMR.InterfaceVersion()` (the fork's `IPhysicsNM`, `VBOX3D_NM_006` needed). The module and the addon of
different releases don't start; `DMR_Problems()` lists what keeps DMR from working (the fork, the GTA IV folder),
`dmr_info` prints it all.

## `dmr.*`

| Function | What it does |
|---|---|
| `dmr.Attach(rag[, opts])` | NM on any humanoid `prop_ragdoll` whose skeleton is mapped (ValveBiped, a known rig, a saved mapping: docs/adr/0030) (`opts.balance = false`: without `bodyBalance`): a body fitted to its model that takes its pose and motion (ADR 0027). A controller, or `nil, reason` |
| `dmr.Detach(rag)` | NM off: the model's own ragdoll is given back as a new ragdoll in NM's pose and motion: `true, newRagdoll` |
| `dmr.Get(rag)` | a ragdoll's controller with NM, or `nil` |
| `dmr.All()` | the controllers of every body with NM |
| `dmr.Messages()` | the names of all NM messages (behaviours) |
| `dmr.MessageParams(name)` | a message's params: `{name, kind, used, doc, default}` (`used = false`: the port takes it but doesn't use it) |
| `dmr.Reactions()` | `{buttons, extra, getup, weapons}`: the Playroom's buttons, additional reactions, get-up modes, weapons |

## The controller

| Method | What it does |
|---|---|
| `c:Send(msg[, params])` | an NM message as the Playroom's console takes it: `c:Send("bodyBalance start=true")` or `c:Send("bodyBalance", {start = true})`. A `Vector` is a game point (turned into NM's frame); for directions (`dir`, `normal`, `bulletVel`, `impulse`, `vel`, `fallOverEdge`) as is; a string `"x,y,z"` is NM's frame untouched |
| `c:React(line)` | a reaction of the Playroom's room layer (a `DMR.FollowReact` line, below) |
| `c:Shot(where[, dir][, weapon])` | a shot: `where` is a part 0–20 or a point (`Vector`), `weapon` one of `pistol deagle smg ak47 sniper shotgun rpg grenade molotov bat knife taser baton` |
| `c:Button(name)` | a Playroom button (`balance`, `catchfall`, `flinch`, `brace`, `relax`, `STOP all`, …, `shot`, `shot back`) |
| `c:Balance()`, `c:CatchFall()`, `c:Relax()`, `c:Stop()` | the buttons `balance`, `catchfall`, `relax`, `STOP all` |
| `c:Flinch([from])` | `upperBodyFlinch` from a point, or the `flinch` button |
| `c:Kill()`, `c:SetHealth(h)` | health 0 (death: `stopAllBehaviours` + `bodyRelax`) / any |
| `c:GetState()` | `{steps, parked, cost, origin, awake, asleep, balancing, fallen, pelvis, head, com, com_v, health, dead, rising, burning, shock, getup}` (m, tables `{x, y, z}`; `parked`: NM taken off by LOD, `cost`: NM's step, ms) |
| `c:GetParam(msg, param)` | a message param's description |
| `c:Origin()` | the start of NM's frame (m, the game's frame; moves with a teleport) |

## Hooks

| Hook | Arguments | When |
|---|---|---|
| `DMR_Attached` | `ent, id` | NM is on |
| `DMR_Detached` | `ent, why` | NM is off (`detached`, or NaN / blow-up: the reason) |
| `DMR_Fallen` | `ent` | NM's balancer gave up |
| `DMR_Feedback` | `ent, name, slot, value` | NM's feedback (after every NM step) |
| `DMR_Died` | `ent` | health 0: the body went limp |
| `DMR_Reaction` | `ent, text` | a log line of the Playroom's room layer (`room  shot (pistol) at part 9 …`) |
| `DMR_Teleported` | `ent, metres` | the game moved the bodies (`PhysObj:SetPos`): NM's frame and its world moved with them |
| `DMR_DeathRagdoll` | `ragdoll, player` | a dead NPC's or player's ragdoll got NM, the killing hit sent to it (`player`: nil for an NPC) |

## Lifecycle (M7, ADR 0021)

| Event | What DMR does |
|---|---|
| removal, `undo`, `gmod_admin_cleanup` | `OnRemove` takes the agent off before the bodies go |
| a map change, quitting | the `ShutDown` hook and `gmod13_close`: agents and the step callback off before the physics environment |
| a Lua restart (auto-refresh, `lua_openscript`) | the module and agents stay (`require` once a Lua state) |
| the duplicator, save/load (`gm_save`) | DMR's fields don't go into the copy (`PreEntityCopy`); a ragdoll's copy gets its own physics back (unfrozen) and NM on it again |
| teleporting the bodies | NM's frame and the world's crop move, the agent and its state aren't reset |
| welds, ropes | NM stays (outside forces, like a grab) |
| a joint part is lost (`RemoveInternalConstraint`) | NM comes off (`DMR_Detached`, "joint is gone") |
| a panic in the module | a native: a Lua error; an agent's step: only that agent comes off (`DMR_Detached`, `panic: …`) |

LOD: farther than `dmr_lod_distance` (3000 units, 0: off) from every player, a body gets the `relax` button, after 1 s NM
is parked (no steps, drives or armature; the agent, health and behaviour are kept) and comes back when a player is
near; lying still for `dmr_lod_still` s (5, 0: off), it's parked until the bodies' first movement, a reaction or a
message. The limit is `dmr_max_agents` (8): ragdolls beyond it stay ordinary ragdolls. Agents are stepped on
`dmr_threads` threads (0: CPU cores less one).

## Death ragdolls (M11, ADR 0028)

An NPC or a player with a humanoid model that dies leaves a server ragdoll with NM on it, in the pose and motion it
died in; the killing hit goes to it as the Playroom's room takes one on a fatally injured body: `health value=49`, then
the hit's reaction (`shot`, `blast`, `melee`, `burn`, `taser`): the ShotTask's dying reaction (reaching for the wound,
`shotRelax`), no getting up, later hits reacting till its health is 0 (the room's death: `STOP all`, `relax`); a death
without a known hit (a fall, a crush, `kill`) is `health value=0` at once. NPCs' ragdolls are made on the server for it (`SetShouldServerRagdoll`)
without the game's push of the killing hit; a player's client death ragdoll is replaced by a server ragdoll of their
model (skin, bodygroups, colour, player colour), the death camera on it as GMod's (over it, 96 units back, the mouse
turns it), one a player (removed at their next death or when they leave). Undo and cleanup take an NPC's corpse in its
place; removed (cleanup, undo, a fade), NM goes with it.

| Convar | Default | What |
|---|---|---|
| `dmr_autoattach_npc` | 1 | NM on killed NPCs' ragdolls |
| `dmr_autoattach_players` | 1 | NM on killed players' ragdolls |
| `dmr_autoattach_max` | 4 | death ragdolls with NM at once (within `dmr_max_agents`): a new one takes NM off the oldest |
| `dmr_autoattach_time` | 30 | seconds a death ragdoll keeps NM, then it's an ordinary ragdoll (0: till the limit takes it) |

NM off a death ragdoll is `dmr.Detach`'s swap: an ordinary ragdoll of the model in its place. A model's first NM body
needs the fork's hulls (~0.9 s): an NPC's or a player's model is fitted and hulled ahead as they spawn, a part a tick.
`DMR_DeathRagdolls()` lists the death ragdolls with NM, oldest first.

## Live bodies (M11.5, ADR 0029)

A humanoid NPC shot and not killed becomes a server ragdoll of its model with NM on it (posed from its bones, moving
as it was), the bullet the room's shot (`shot at= dir= weapon= part=`); the NPC itself is hidden meanwhile (not drawn,
not solid, not thinking or targeted), keeping its health, relationships and weapon. Hits on the ragdoll are the NPC's
(scaled for the hitgroup by the gamemode, `ScaleNPCDamage`). Once the room's ShotTask is over and the body has settled
(standing, or lying still), or after `dmr_live_time`, the NPC stands where the body is, facing as it did, and the ragdoll
goes; killed meanwhile, that ragdoll is its death ragdoll (fatally injured, as above).

`dmr_drunk` (the DMR window's "Drunk (you)"): the player becomes a ragdoll of their model with the Playroom's `drunk`
button on it, the camera on the body as the death camera (the mouse turns it), their own moves and buttons ignored;
`dmr_drunk` again stands them up where the body is. Hits on it are theirs; dying drunk, it's their death ragdoll.

| Convar | Default | What |
|---|---|---|
| `dmr_live_npc` | 1 | shot NPCs become ragdolls with NM, then get up |
| `dmr_live_max` | 4 | NPCs that are ragdolls at once (within `dmr_max_agents`); more take shots as the game does |
| `dmr_live_time` | 15 | seconds at most an NPC stays a ragdoll |

`DMR_LiveBodies()` lists `{owner, rag, kind ("npc", "drunk"), since}`; hooks `DMR_LiveStarted(owner, rag)` and
`DMR_LiveEnded(owner)`. `DMR.FollowRoom(id)` has `shot` and `blast`: the room's ShotTask and explosion task run.

## Reactions (`DMR.FollowReact(id, line)`, `dmr_react`)

Points are `x,y,z` in metres, the game's frame. What the Playroom does on each: `module/src/follow/room.rs` (the port
of `Sim`).

| Line | The Playroom's reaction | What NM gets |
|---|---|---|
| `shot [part=k] [at= dir=] [weapon=] [fatal=1] [more=k,k]` | `fire` | the weapon's damage (`ped::damage`, `weaponinfo.xml`), the bullet's `applyImpulse` (`shottask::bullet`), ShotTask with the weapon's taskparams set (`weapon_set`); `more`: the other pellets |
| `blast at=` | `blast_on` | `applyImpulse` on each part within 5 m (15 m/s · mass · (1 − d/5)), damage 250 · (1 − d/5), the explosion task for 6 s |
| `burn` | `burn` | 25 damage a second; NM's OnFire task (`fire_reaction`) |
| `taser [part=k] [at=]` | `zap` | `electrocute start=true stunMag=0.4 largeMult=2` for 2.5 s |
| `melee damage= at= impulse= head=` | `land` (Hero's blow) | `applyImpulse`, the game's melee task: flinch (`MELEE`/`MELEE_PASIVE`) or balance |
| `nearmiss from= dir=` | `near_miss` | within 1.5 m of the head or chest: `bodyBalance`, `upperBodyFlinch`, `headLook`; lying: `bodyFoetal` (additional reactions); `dive_shot`: a dive |
| `car impulse= at= [id=] [player=1]` | `car_react` | damage `bump::damage`, the task `bump::react`: brace / rollup / balance / relax |
| `prop at= impulse=` | `obj_hit` | damage 0.6 per N·s (1–35), a limb injury (`injury`) |
| `thrown from= vel=` | `throw` | `headLook` (`look`), `upperBodyFlinch` (`avoid`) |
| `dropped` | `fall` | `smartFall` (additional reactions) |
| `button name=<button>` | `button` | the button's messages (`Sim::lines`); `STOP_all` is `STOP all` |
| `health value=`, `kill` | — | health; `kill`: a fatal shot |

The game is turned into reactions by itself (`addon/lua/dmr/reactions.lua`): `EntityTakeDamage` (bullets → `shot`, one
tick's pellets → one `shot` with `more`, a blast → `blast`, fire → `burn`, a blow → `melee`, a stun gun → `taser`),
`PhysicsCollide` (a car → `car`, a prop → `prop`), `EntityFireBullets` (a miss → `nearmiss`), the physgun and gravity
gun (`thrown`, `dropped`). The push of a bullet or blast is the Playroom's `applyImpulse`, so Garry's Mod's own damage
force on the body is zeroed.

## The Playroom's switches (convars, the Playroom's defaults)

| Convar | Default | What |
|---|---|---|
| `dmr_tasks` | 0 | the game's tasks: NM → animation as a task ends |
| `dmr_look`, `dmr_avoid` | 1, 0 | looks at what's thrown / dodges it |
| `dmr_getup` | 0 | getting up: 0 the game's clip, 1 muscles (NMPose along the clip), 2 doesn't get up, 3 the learned network (not ported) |
| `dmr_extra_reactions`, `dmr_extra_pick` | 0, 255 | v0.2.1's additional reactions and which of them (bits: windmill, curl up, curl up under fire, look at who hit him, tumble off a ledge, smartFall when dropped, dodge from above, brace on a wall) |
| `dmr_fire_reaction` | 1 | OnFire while burning |
| `dmr_dive_shot`, `dmr_dodge_gap` | 0, 3 | a dive when shot at again |
| `dmr_weak`, `dmr_god` | 0, 0 | weak weapon sets, infinite health |

Bench scenarios (`dmr-check fit`) always run on the Playroom's defaults (only a scenario's `!set` changes them).

## Tuning (ADR 0009)

`dmr_tuning 0|1`, `dmr_tuning_profile <name>`; profiles are `garrysmod/data/dmr/profiles/*.json`
(`{name, messages = {[message] = {[param] = "value"}}, taskparams = {["task set param"] = number}}`), differences from
the Playroom only; "Playroom" is empty and can't be changed. A profile's values are added to the messages that start a
behaviour or change its params, unless the line sets them itself; taskparams go over the game's file.
`dmr_tuning_status`: the active values. The menu is the DMR window (ADR 0022): `dmr_menu`, the key from `dmr_menu_key`,
the DMR icon in the context menu (C) or "Utilities → DMR → Ragdolls (Delirium)".

## The DMR window (ADR 0022)

Tabs: Ragdolls (NM on/off the ragdoll you look at and every ragdoll, a citizen with NM, the fit under the crosshair,
auto-attach on spawned ragdolls, death ragdolls and live NPCs, the drunk mode, NM's bodies drawn over the ragdolls, the
window's key), Reactions (the Playroom's switches, get-up, additional reactions, try-outs), Tuning, Task params,
Performance (the M7 convars), Debug (the versions and problems, buttons for `dmr_status`, `dmr_selftest`, `dmr_data`,
`dmr_info`, `dmr_tuning_status`, `dmr_nm_messages`, `dmr_nm_parts`, `dmr_nm_joints`, an NM message line to the ragdoll
you look at, `dmr_react_log`). The help pane on the right explains what's under the cursor.

## The DMR tool (ADR 0023)

Toolgun → Tools tab → "DMR". The mode (`dmr_tool_mode`): `attach`, `react`, `behaviour`, `target`, `inspect`, `kick`,
`health`; what the left and right buttons and R do is in the tool's hint and ADR 0023 (its throw and pins were
Homer's: gone with him). Options: `dmr_tool_reaction`, `dmr_tool_weapon`, `dmr_tool_damage`, `dmr_tool_impulse`,
`dmr_tool_behaviour`, `dmr_tool_target` (`look|point|lean|flinch|grab`), `dmr_tool_health`. Server:
`dmr_tool_everyone 0|1` (superadmins only by default). Lua: `DMR_ReactAt(ent, kind, rest, hit, from, impulse)`, the
Playroom's reaction at a point.

## Console

| Command | What |
|---|---|
| `dmr_attach` | NM on the humanoid ragdoll the player looks at, as it lies or falls |
| `dmr_detach` | NM off the ragdoll the player looks at (a fitted one moves on with its own physics) |
| `dmr_detach_all` | NM off every body |
| `dmr_ragdoll [model]` | a model's ragdoll (default male_07) in front of the player on a fitted body, NM, `bodyBalance` |
| `dmr_fit [model]` | the model's fit: its rig, height, mass, warnings (kept by the model, its checksum and mapping) |
| `dmr_rig [model]` | what the model's skeleton is to DMR: its rig (ValveBiped, Biped, Mixamo, Unreal, TF2, VRoid, MMD, `by names`, `mapped`, `unknown`, `not humanoid`), the bone at each NM joint, what's missing, the reader's report (docs/adr/0030) |
| `dmr_rig_view 1` (client) | NM's bodies (capsules) drawn over the ragdolls with NM near you (superadmins) |
| `dmr_autoattach_ragdolls 1` | NM on humanoid ragdolls spawned from the spawn menu (off by default) |
| `dmr_send <line>` | an NM message to the ragdoll under the crosshair (from the server console: to all) |
| `dmr_react <reaction> [key=value …]` | a reaction of the ragdoll under the crosshair; without `at=`, the crosshair's point |
| `dmr_status`, `dmr_tuning_status` | the agents (NM's step cost, threads, LOD), tuning |
| `dmr_info` | versions (module, addon, Delirium, `IPhysicsNM`) and what's wrong |
| `dmr_game_dir "<folder>"`, `dmr_data`, `dmr_selftest` | your GTA IV folder, NM's data from it, NM's self-test (docs/data-setup.md) |
| `dmr_react_log 1` | print the room layer's log |

A model's rig made by hand is `data/dmr/skeletons/<model path without models/ and .mdl>.json` (docs/adr/0030; its
`dms_skeleton` format key as before the split): a bone per NM part, joints moved, parts' sizes, masses and placement on
the model. The addon reads it again as the file changes and puts NM again on the model's ragdolls by it; copy the file
to share it.

The Playroom's face, skeleton and debug overlay were drawn on Homer's model's bones: they're the golden project's.
