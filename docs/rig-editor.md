# The rig editor (`dms-rig`)

A model whose skeleton DMR doesn't map by itself (or maps, but fits badly) gets a rig of its own: a file
`garrysmod/data/dmr/skeletons/<model>.json` ([ADR 0030](adr/0030-workshop-skeletons.md)). `dms-rig` makes it by the mouse
over the model, with the module's own fit, and tries the body on VPhysics-Box3D itself
([ADR 0031](adr/0031-rig-editor.md)). The game can stay open beside it: the addon reloads a rig file within a second of
its change and gives the model's ragdolls with NM their new body.

```powershell
cargo build --release -p dms-rig
.\target\release\dms-rig.exe                                   # Garry's Mod from Steam, GTA IV from data\dmr\game_dir.txt
.\target\release\dms-rig.exe --model models/player/foo.mdl     # a model at once
```

It needs your GTA IV (NM's body is Homer's, fitted: `--game`, else `garrysmod\data\dmr\game_dir.txt`, else
`fit\game_dir.txt`) and Garry's Mod with the fork installed (`scripts\install.ps1 -Fork`; `--gmod`, `--vphysics`). It
reads models where the game has them (its VPKs, loose files, the Workshop's GMAs) and writes only the rig file.

## Rig

- **Models** (left): filter by words, *Workshop* for the Workshop's only; *Classify shown* tags each with its rig
  (`ValveBiped`, `Biped`, `by names`, `! unknown` …). Click one to open it: its mesh in its own pose, NM's 21 parts over
  it, its bones as lines.
- **Mapping** (right): a bone for each NM part, from the list or by *pick* and a click on the bone in the view.
  NM's Spine1–3 are always spread between the spine and the neck. A mapping equal to the model's own isn't written.
- **The head's face**: a yellow arrow out of NM's head shows where it looks (a capsule has no face of its own): Homer's
  forward as he stands, carried by the head's frame as shown, its turn edits included.
- **The mouse** in the view: click a part or a joint to select it. Drag a joint (yellow; violet for the spread spine
  joints) to move it. Right button orbits, middle (or Shift + right) pans, the wheel zooms.
- **The gizmo** on the selected part, one tool at a time (the second toolbar): *Select* (Q, none), *Move* (W: arrows
  along the axes, the white square on the screen's plane), *Rotate* (E: a ring about each axis, the outer grey one about
  the view), *Scale* (R: the blue squares change a capsule's radius and length or a box's extents), *All* (T). Its axes
  are the part's own (*Part*: a capsule's green axis is its length, along its bone), the model's bone's (*Bone*) or the
  world's; the rings turn the part about its centre or its joint. Move moves the part on its bone, or the joint when a
  joint was clicked last. Ctrl snaps: moves to 1 cm, sizes to 5 mm, turns to 5°; Esc cancels a drag.
- **Symmetry** (M): an edit of an arm's or a leg's part or joint is made on the other side too, across the model's plane
  between its hips: the joint's move mirrored, the size and mass copied, the part's placement its mirror image.
- **Snap to bones** (B: the selected part; Shift+B: all): Homer's proportions laid on the model's bones, as thick as its
  mesh ([Snap](#snap) below). The result is ordinary edits (sizes, the parts moved and turned on their bones): undo,
  symmetry and the file as for any other; the masses stay. The status line gives the factors used and why a part was
  left as the plain fit has it.
- **IK** (I): dragging a wrist's or an ankle's joint carries the elbow or knee (the shoulder or hip stays, the limb's
  lengths kept, bent as it was); dragging an elbow or a knee turns it about the limb's line.
- **Numbers**: the selected part's joint offset (units), size (m), mass (kg), offset (m) and turn (degrees), editable
  by dragging or typing.
- **File**: *Save* (Ctrl+S) writes `data/dmr/skeletons/<model>.json`; *Import* reads a rig file from anywhere (a rig sent
  by someone), *Paste JSON* takes its text, *Copy JSON* gives yours; *Model's own* clears the mapping and edits. Ctrl+Z
  and Ctrl+Y undo and redo every change.

## Snap

Snap (B, Shift+B, `--snap`) is our own placement of NM's parts on a model, not NM's fit. It starts from the plain fit
(the model's joints, no size or placement edits: Homer's parts scaled by the model's bone lengths). The fit builds NM's
body in Homer's stance (ADR 0027) and turns each part back onto the model's bind pose by its segment, so a part shows up
to ~20° off its bone where Homer's own shape isn't along his joints, and the fit's hands may stand far off the wrist.

**What it measures.** Each part's points: the model's ragdoll hulls (its `.phy`: one simple convex solid per physics
bone, its maker's own body without holsters, coats or fingers) sampled about a centimetre apart, where the part has a
solid that lies on its mesh (a limb's cross-section's radius 0.6–1.2 of the mesh's and its centre within half of it;
another part's middle within half the mesh's spread, its spread 0.6–1.2 of the mesh's: thinner, since clothes and gear
make a mesh fat, hardly fatter, since many models borrow another's `.phy`, a citizen's on a fast zombie, hands where
another model's are); else the mesh's drawn vertices (a `.vvd` also holds the other body groups'), each the part's whose bones carry
most of its weight (a twist bone's share adds to its limb's), firm where they carry 75%; the spine's by the nearest of
its four segments. Every measure is robust:

- a limb's cross-section (the middle 20–80% of its bone) is its inner shell round the bone (per sector round it, the
  points near its nearest: a holster or a pouch outside, however dense, is left out), a circle fitted to it with the
  points off it left out by their median absolute deviation; it counts only when it lies all round the bone (6 of 8
  sectors, the bone inside it);
- the torso's depth in each spine segment's slab (all the torso's points), the pelvis' round its capsule, the head's
  extents along its shape's axes (the middle of the three: a hat or a helmet makes one), all medians with the outliers
  left out;
- a foot's way is its toes', checked against the long axis of its points below the ankle (a boot's top is a ring);
  a hand's is its fingers' (the farthest bone below that carries any mesh: not an attachment).

**How it uses them.** Thickness is Homer's times three bounded factors: the body's (the middle of every measure but the
head's, 0.5–1.8), its group's (legs, arms, torso, pelvis: 0.75–1.33 of the body's; the head, neck, hands and feet
0.6–1.4 of Homer's, since armour and coats thicken a body, not its head or boots) and a limb's or spine segment's own
(0.7–1.15 of its group's; a woman's shins are thinner to her thighs than Homer's). The sides of a pair are one: their
own factors' mean when they agree within 20%, the thinner when they don't (a rolled sleeve; a part a little thin is
better than one fatter than its mesh), the group's when they're twice apart (one of them is wrong). The clavicles take
the thinner of the arms' and the torso's. No part is thinner than 0.35 of Homer's. Placement:

- a long part (a thigh, shin, upper or lower arm, clavicle) lies along its bone at Homer's length, its cross-section
  centred on the circle's centre, both sides' mirror images averaged, at most half its thickness off the bone;
- a foot's or hand's box lies along its way (both sides' made mirror images where they agree), as long as its points
  (0.8–1.25 of Homer's), as wide and thick as their core (10–90%), centred in them; a foot's sole on theirs;
- the head is turned upright as Homer's head stands, facing the model's way: the fit turns it with its neck segment,
  and a model whose neck leans further forward than Homer's in its bind pose (HL2's citizens, Kleiner, the Combine:
  33° against his 15°) got its head bowed 18°. It is moved into the middle of its points' box in that frame (each
  axis' 3–97% span, not the distances from a median, which the face's many vertices pull forward), at most its
  reach off the fit's, and sized by the box's extents (the middle of the three);
- the pelvis, the spine and the neck keep their way and place.

A part whose data are too few or contradict each other stays as the plain fit has it, and the status line says why
(the first note, and how many more; `--snap-report --verbose` prints them all). A hand without mesh is laid from its
wrist along its bones (the fit's may be far off it); a foot without (a peg leg) stays.

**The report.** `--snap-report <text[,text…]>` snaps every complete humanoid whose path has a text and scores each part
against its own bone and mesh (the drawn vertices, not the hulls: the mesh is what the player sees), with explicit
bounds (`tools/dms-rig/src/report.rs`):

| score | a fault past |
|---|---|
| a long part's axis to its bone; a foot's or hand's to its mesh's way | 20° |
| the head's face to the model's way (its hips' line across the up) | 10° |
| thickness to the mesh's (a limb's radius to its cross-section's circle; a box's extents to its points' 5–95%) | 0.55–1.4 |
| the surface standing out of the mesh (80th percentile; a limb's caps and a box's edges left out) | half its thickness, at least 2.5 cm (a box: half its middle extent) |
| a capsule's own vertices inside it (a fat capsule swallows them) | 85% |
| a limb's centre off its section's circle; a box's off its points' middle (a foot's bottom off its sole) | 0.6 of its thickness |
| a centre off its bone's line | 1.5 of its thickness, at least 4 cm |
| the sides: thickness, centres from their joints mirrored, axes mirrored | 15%, 3 cm, 12° |
| thickness to Homer's | 0.345–2.2 |
| `--sim <steps>`: after a drop on the fork, the fastest part; the farthest from the body's middle | 0.5 m/s; 1.6 m |

`--plain` scores the plain fit instead; the exit code is 1 when any model has faults. Give `--sim` 900 steps: a body
may take more than 300 to stop rocking. On the 259 complete humanoids of a Garry's Mod with HL2, its episodes, CS:S
and some Workshop models:

| | models with faults | faults | after a drop |
|---|---|---|---|
| the plain fit | 258 of 258 | 3659 | 6 still rocking after 300 steps |
| the first Snap | 259 of 259 | 1097 (650 standing out, 262 swallowing, 90 asymmetric) | the Advisor flew apart (56 m/s) |
| this Snap | 43 of 258 | 113 | all at rest after 900 steps (zombie_soldier after 1800) |

What remains, and why it stays:

- HL2's women's hands (18 models, 36 faults): a box over a cupped hand stands 3.0–3.2 cm out where 2.5–2.8 is the
  bound; its size is the hand's (×0.96–1.15).
- The zombies whose head is their headcrab's bone (`zombie/classic`, `zombie_soldier`, `HC_Head_Bone`): the rig maps the
  head there and the spine runs to it; make their rig by hand.
- Eli's peg leg, the skeleton (bones thinner than 0.35 of Homer's), the old `player.mdl`'s bent fingers (24–26°).
- Sides 3.5–6.7 cm off each other's mirror where the model's own bind pose isn't symmetric (a hand turned, an arm
  raised: Big Boss's ragdoll, EFT's PMCs, Mossman, Chell).
- Single parts a little past a bound: G-Man's hand on his briefcase (thinner than it, ×0.45), a clavicle under a vest
  (4.4 cm out), feet and necks 2.5–3.6 cm out on thin models (Chell, the T-800, the fast zombie, the charple).

## Simulate

The fitted body made by the fork as the module makes it (`CreateBody`, `CreateJoint`, NM's pivots, frames, limits,
inertia, materials and self-collision pairs) on a concrete floor: *Stand*, *Drop*, *Throw*, *Kick* (every part given
3 m/s sideways and a spin), and drag any part with the left button. Gravity is NM's 9.8 m/s² (what the body feels in
the game under NM) or Source's 600 units/s². The model's mesh follows the parts as the game poses it; *Homer's own
body* simulates the template's own body (NM's build unfitted, the golden project's) instead of the fit.

**NM: bodyBalance** stands the body and puts NM on it as the module's agents drive one ([ADR 0032](adr/0032-nm-in-rig-editor.md)):
the Playroom's agent in FOLLOW, stepped from the environment's step callback (1/60 s NM steps over the 0.015 s physics
steps), its drives, armature, balancer impulses and gravity's compensation on the bodies, all of it the module's own
code (`dms_vphysics::follow`), with the Playroom's balance button pressed (`bodyBalance start=true`). *Push* is the
Playroom's poke: an impulse into the spine (part 9) away from the view, 8 N s by default; dragging a part is the
Playroom's grab. The panel shows whether he's balancing or fallen, his health and the shot task, NM's messages and
feedback, and why NM came off (a blow-up or a state that isn't finite).

Under NM ([ADR 0033](adr/0033-room-shot-shared.md)):

- **Shoot** is the room's shot, the game's bullets' (the Playroom's `shot` button; *from behind*: its `shot back`):
  the weapon's damage on his health (weaponinfo.xml's, from your GTA IV), the limb's injury mask, the bullet's
  impulse and the ShotTask with the weapon's taskparams set, then its messages every other step; at health 0 he goes
  limp (`STOP all`, `relax`). Pick the weapon (the Playroom's: pistol, ak47, shotgun, ...) and the part (Spine2 by
  default). It's the module's code (`dms_vphysics::follow::shot`).
- The Playroom's **buttons** that only send messages (`balance`, `windmill`, `pedal`, `writhe`, `rollup`,
  `catchfall`, `highfall`, `stairs`, `relax`, `stayup`, `foetal`, `STOP all`) and a **message line**: any of NM's
  messages as the Playroom's console takes it (`<message> [param=value ...]`, e.g. `bodyBalance start=true` or
  `armsWindmill start=true`), sent with Enter or *Send* (Up brings the last line back). A wrong line is reported in
  the panel with the console's words; the tuning profile is off, as in the Playroom (ADR 0009).

The rest of the room's reactions (its tasks, get-ups, `in_air`, the additional reactions, blasts, fire, the taser,
cars) are the module's and aren't in the editor.

`--snap-report <models> --sim 600 --nm balance [--push <N s> | --shoot <weapon>]` does it for each model: stood under
bodyBalance, pushed (8 N s by default) or shot in the chest from the front halfway, and the same build in LEAD (NM's
own physics, the same blow) for comparison; `homer` among the texts adds Homer's own body. A fault is a pelvis below
0.8 of its standing height (or NM coming off). With 600 steps and a 40 N s push: Homer and `humans/group01/male_07`
stand and take it (male_07 steps 0.44 m); `kleiner`, `alyx` and `combine_soldier` go down after 1.2–2.1 s though they
stand in LEAD (a difference of the fork's physics from NM's on those bodies, the fork's to fix); every plain fit goes
down. Shot with the pistol, Homer staggers 1.8 m and stays up (health 175, as in LEAD: the pelvis at 0.887 m in both),
with the AK47 2.05 m; `humans/group01/male_07` goes down 3.2 s in, though it stands in LEAD, and
`player/group01/male_07` stays up.

## For scripts

`--classify <text>` prints the class of every model whose path or source contains the text (no window);
`--bones <model>` its bones; `--select <part>` and `--tool <name>` set the view up (with a screenshot, the view frames
the selected part), `--yaw <degrees>` turns it round the model, `--snap` snaps all the parts at once;
`--screenshot <file.png> [--sim <steps>]` saves the window once drawn (after that many physics steps) and quits; with
`--nm balance` the body is stood under NM's bodyBalance instead of dropped, `--push <N s>` pokes it halfway (or
`--shoot <weapon>` shoots it), and
`--homer` simulates Homer's own body;
`--snap-report` scores Snap ([Snap](#snap)).
