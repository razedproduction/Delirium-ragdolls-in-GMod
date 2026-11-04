# Installing DMR

DMR puts Delirium, an active ragdoll in the NaturalMotion style, on Garry's Mod's own ragdolls: HL2's citizens,
combine, Alyx, player models and Workshop humanoids get NM's muscles, so they balance, stumble, brace and fall on their
own and react to shots, blasts, fire and blows as the Delirium Playroom's body does. Killed NPCs and players fall as NM
ragdolls, shot NPCs stagger and get up. Each model keeps its look.

DMR comes in two parts:

- **the DMR addon on the Steam Workshop** (the Lua part; its page is linked from the release);
- **`DMR-<version>-win64.zip` from GitHub Releases** (the physics and the module, with `install.bat`): the Workshop
  doesn't take these files.

Both have to be the same release: Steam updates the addon by itself, so install the new zip when a new DMR comes out.
DMR can be installed beside the Delirium Playroom (DMS, Homer): both use the same `vphysics.dll`.

## What you need

- **Garry's Mod launched in 64-bit** (the stable branch has it: the folder `GarrysMod\bin\win64`), on **Windows**.
  32-bit, Linux and dedicated servers aren't supported (see the FAQ).
- **Your own copy of GTA IV**: the PC version (the folder with `GTAIV.exe`) or Xbox 360 (the folder with
  `default.xex`). DMR reads NM's character and the body it fits to every model from it. No game files come with DMR
  and none are copied anywhere: everything is read from your copy on your machine.
- Any x86-64 CPU: **AVX2 isn't needed**.

## Install

1. Subscribe to the DMR addon on the Workshop.
2. Close Garry's Mod.
3. Unpack `DMR-<version>-win64.zip` anywhere.
4. Double-click `install.bat` in the unpacked folder and choose **1**. It finds Garry's Mod through Steam (or asks for
   its folder) and asks for your GTA IV folder (the Playroom's, if DMS knows it already); you can drag a folder onto
   its window.

| Choice | What it does |
|---|---|
| 1. Install or update DMR | the files below, then your GTA IV folder |
| 2. Set your GTA IV folder only | another GTA IV folder |
| 3. Uninstall DMR | see [Uninstall](#uninstall) |

| What | Where |
|---|---|
| `bin\vphysics.dll` (VPhysics-Box3D, the physics DMR needs) | `GarrysMod\bin\win64\vphysics.dll`; Steam's own is kept once as `vphysics.dll.original` |
| `bin\gmsv_dmr_win64.dll` (the DMR module) | `GarrysMod\garrysmod\lua\bin\` |
| your GTA IV folder | its path into `garrysmod\data\dmr\game_dir.txt` |

### By hand

Subscribe to the Workshop addon, copy the two dlls as in the table (keep a copy of the original `vphysics.dll`), and in
Garry's Mod `dmr_game_dir "<GTA IV folder>"`.

## First steps

Start a local game (single player or a listen server you host) on any map.

- `dmr_ragdoll` in the console (`~`): a citizen stands in front of you on NM, balancing (`dmr_ragdoll <model>` for
  another model).
- Spawn any humanoid ragdoll from the spawn menu: NM takes it as it lands (`dmr_autoattach_ragdolls`); `dmr_attach`
  and `dmr_detach` put NM on or off the ragdoll you look at.
- `dmr_menu` opens the **DMR window** (bind a key in its Ragdolls tab, or hold `C` and click the DMR icon): ragdolls,
  NPCs' deaths and live reactions, the drunk mode (`dmr_drunk`), the Playroom's reactions and switches, behaviour
  tuning, performance, and a Debug tab with versions and checks.
- Toolgun → Tools → **DMR**: NM on or off a ragdoll, reactions where you hit it, its behaviours, a target to look at or
  point at, an inspector, a kick, health.
- A model DMR doesn't know: make its rig in the rig editor (`dms-rig`, [rig-editor.md](rig-editor.md)), saved in
  `garrysmod\data\dmr\skeletons`.
- `dmr_selftest` checks that NM's data reads from your GTA IV; `dmr_info` shows the versions and anything wrong.

The server's settings (console commands, the window's server tabs, the tool) are for superadmins: in single player
and on a listen server you host, that's you.

## FAQ

**The console says "vphysics.dll isn't VPhysics-Box3D" / DMR is disabled.**
Steam's *Verify integrity of game files* (and some Garry's Mod updates) put the original `vphysics.dll` back. Run
`install.bat` again and choose 1.

**"gmsv_dmr isn't installed".**
The module isn't in `garrysmod\lua\bin`: run `install.bat` (choice 1).

**Nothing of DMR in the game: no `dmr_ragdoll`, no DMR window.**
The Workshop addon isn't subscribed or downloaded yet: subscribe and restart Garry's Mod.

**"NM's data isn't loaded" / "No GTA IV folder".**
DMR doesn't know where your GTA IV is: `install.bat` (choice 2) or, in the console, `dmr_game_dir "<GTA IV folder>"`
and restart Garry's Mod (the data loads once a game). The DMR window's Debug tab lists what's missing.

**"gmsv_dmr_win64.dll is X but the Lua addon is Y".**
The module and the Workshop's addon come from different releases: download the zip of the addon's release and run
`install.bat` again.

**A model isn't taken ("not a humanoid", "no bone for …").**
DMR knows ValveBiped, Biped, Mixamo, Unreal, TF2, VRoid and MMD skeletons and guesses others by their bones' names;
animals and robots without a humanoid chain aren't supported. A humanoid it can't map gets a rig in the rig editor.

**I have the Delirium Playroom (DMS) too.**
Both load side by side. Put NM on bodies with one of them at a time: the physics steps NM's bodies for one module, so
the other's stop being driven while it does.

**"Run as administrator" / can't write `vphysics.dll`.**
Garry's Mod's folder isn't writable for you: right-click `install.bat` → *Run as administrator*.

**Does it work on a dedicated server or Linux?**
No. Garry's Mod's x86-64 dedicated server is built against newer physics interfaces than the game, and VPhysics-Box3D
doesn't run there; there's no Linux build either. DMR runs in the game: single player or a listen server.

**Do other players on my listen server need DMR?**
The physics and the module are the host's only; the addon's client part (the window, the tool, the models' poses)
comes to them from the server as Garry's Mod sends any addon's Lua, and the models are the game's or the Workshop's.

**Does DMR change my other games or Garry's Mod's physics?**
VPhysics-Box3D replaces Garry's Mod's physics engine for everything (a Box3D port of VPhysics); `sv_gravity` and the
rest of the game aren't touched. To go back: [uninstall](#uninstall).

## Uninstall

`install.bat`, choice 3: removes the module and puts Steam's `vphysics.dll` back (unless the Delirium Playroom is
installed: it needs it). Then unsubscribe from the DMR addon on the Workshop. `garrysmod\data\dmr` (profiles, rigs, the
GTA IV path) stays; delete it by hand if you like.

## For modders

The Lua API (`dmr.Attach`, controllers, hooks, reactions): `docs/api.md` in the repository
(https://github.com/razedproduction/Delirium-ragdolls-in-GMod). Building from source: the repository's README.
