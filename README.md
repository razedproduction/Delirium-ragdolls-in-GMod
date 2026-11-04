# Delirium Ragdolls in GMod

Delirium (NaturalMotion-style active ragdolls) on Garry's Mod's **own ragdolls**, NPCs and Workshop models, in
**FOLLOW** mode: NM's own body is fitted to the model's skeleton (sizes, masses), the
[VPhysics-Box3D](https://github.com/Asphaltian/VPhysics-Box3D) fork makes it natively under the model, and every
physics step Delirium returns forces and torques for its parts while the model takes its pose. Citizens, combine, Alyx,
player models and any humanoid whose skeleton DMR knows (ValveBiped, Biped, Mixamo, Unreal, TF2, VRoid, MMD, or a rig
made in the rig editor) balance, stumble, brace and fall, react to shots, blasts, fire and blows; killed NPCs and
players fall as NM ragdolls, shot NPCs stagger as one and get up.

This is the ragdolls side of the [Delirium Playroom in GMod](https://github.com/razedproduction/Delirium-physics-in-GMod)
(DMS), which keeps Homer held to the Playroom one to one. DMR builds on its core (NM's data from the game, the fork and
its bindings) and installs beside it: [docs/adr/0036](docs/adr/0036-split-from-golden.md).

> The project is developed with [Claude Code](https://claude.com/claude-code).

## Playing

You need Garry's Mod's **stable branch** launched in x64 on Windows and **your own copy of GTA IV** (PC or Xbox 360):
NM's data is read from it, nothing of the game is shipped. Download `DMR-<version>-win64.zip` from
[Releases](https://github.com/razedproduction/Delirium-ragdolls-in-GMod/releases) and follow
[docs/install.md](docs/install.md): installation, first steps (`dmr_ragdoll`, `dmr_menu`), FAQ.

- [docs/install.md](docs/install.md): installation and FAQ.
- [docs/api.md](docs/api.md): the Lua API for mods, console commands, convars.
- [docs/rig-editor.md](docs/rig-editor.md): the rig editor for models whose skeleton DMR doesn't know.

## Developing

- [ROADMAP.md](ROADMAP.md): the plan.
- [docs/adr/](docs/adr/README.md): architecture decisions.
- [docs/bench.md](docs/bench.md): the bench in the game and `dmr-ctl`; [fit/README.md](fit/README.md): `dmr-check`,
  a fitted body against NM.

Building (Windows, Garry's Mod's x86-64 branch only). Next to this checkout: Delirium's repository (the SDK,
[docs/adr/0001](docs/adr/0001-rust-module-sdk-rlib.md)) and the golden checkout `DMS in Gmod` (the core and the fork,
read only, [docs/adr/0036](docs/adr/0036-split-from-golden.md)):

```powershell
.\scripts\build.ps1             # -> dist\gmsv_dmr_win64.dll
.\scripts\install.ps1 -Fork     # the module, the addon and the golden checkout's fork into Garry's Mod
.\scripts\package.ps1           # built and packed: dist\DMR-<version>-win64.zip
```

## Licence

GPL-3.0-only, see [LICENSE](LICENSE). The VPhysics-Box3D fork shipped in a release (built in the golden checkout, from
[Asphaltian/VPhysics-Box3D](https://github.com/Asphaltian/VPhysics-Box3D)) is MIT. Delirium, Box3D, Qhull, the Source
SDK and the games: [THIRD_PARTY.md](THIRD_PARTY.md).
