# Licences and credits

## DMR

DMR (the `gmsv_dmr_win64.dll` module, the Lua addon and the tools in this repository) is licensed under the **GNU
General Public License version 3 only** (GPL-3.0-only), see [LICENSE](LICENSE). The source of every release is the
repository's tag of that version: https://github.com/razedproduction/Delirium-ragdolls-in-GMod. It builds on the core
of the Delirium Playroom in GMod (DMS, GPL-3.0-only too: `dms-game`, `dms-vphysics`, the fork),
https://github.com/razedproduction/Delirium-physics-in-GMod, at the state each release names.

## Delirium

`gmsv_dmr_win64.dll` is built with **Delirium** (the `delirium`, `delirium-data` and `delirium-sdk` crates), by PVCPC,
licensed under GPL-3.0-only, linked in statically. Its source, the version these binaries are built from:
https://github.com/PVCPC/delirium-physics, tag `v0.6.0`.

Nothing else of Delirium's repository is in DMR: none of the Playroom's assets listed in its `CREDITS.md` (the taxi,
the sky, textures, the font, the Portal and Skate 3 code), and no trained networks (`getup.net`, `walk.net`,
`learned-preview/`).

## VPhysics-Box3D

`vphysics.dll` is VPhysics-Box3D (https://github.com/Asphaltian/VPhysics-Box3D) by Asphaltian, with DMS's `IPhysicsNM`
extension (the DMS repository's `vphysics/` folder), licensed under the **MIT License**: `licenses/VPhysics-Box3D.txt`.
It contains:

- **Box3D** by Erin Catto (https://github.com/erincatto/box3d), MIT License: `licenses/Box3D.txt`. DMS's changes are the
  patches in its `vphysics/cmake/patches/`.
- **Qhull**, Copyright (c) 1993-1998 The Geometry Center, University of Minnesota: `licenses/qhull.txt`.
- Code from the **Source SDK 2013** (https://github.com/ValveSoftware/source-sdk-2013), Copyright (c) Valve
  Corporation, under the Source 1 SDK License: `licenses/SourceSDK2013-LICENSE.txt` and
  `licenses/SourceSDK2013-thirdpartylegalnotices.txt`. DMR is free of charge.

## Games

- **Grand Theft Auto IV** is Rockstar Games'. No files of it, and nothing made from them, come with DMR: NM's data is
  read from your own copy as the game runs. Since Delirium v0.2.6 even the engine's numbers are read from your own
  `GTAIV.exe` (the Steam version) or the Xbox 360 disc's `default.xex`.
- **Garry's Mod** is Facepunch Studios'; the Source engine is Valve's; the models DMR drives are their makers'
  (Valve's, or the Workshop's authors'), read where the game has them and never copied.
- NaturalMotion and euphoria are NaturalMotion's. DMR and Delirium aren't affiliated with or endorsed by Rockstar Games,
  Take-Two Interactive, NaturalMotion, Facepunch Studios or Valve.
