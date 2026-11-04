//! Skeletons beyond ValveBiped (ROADMAP M12, docs/adr/0030): which of a model's bones is each of NM's joints. Known
//! rigs by their bones' names (ValveBiped, 3ds Max's Biped, Mixamo, Unreal's mannequin, TF2/SFM, VRoid, MMD), then a
//! guess by the names' words and the hierarchy, or a mapping the player made in the DMS window. NM's Spine1 to Spine3
//! are never mapped: they're spread between the spine and the neck as Homer's are (ADR 0025), whatever the rig has
//! there.

use delirium::prelude::*;

use crate::M;
use crate::skeleton::Skeleton;

/// NM's parts in part order (`delirium_data::nm_asset::PARTS`): the mapping's keys.
pub const PARTS: [&str; 21] = delirium_data::nm_asset::PARTS;

/// The parts a rig may lack: the neck and the clavicles (put between their neighbours), and Spine1 to Spine3 (never
/// mapped).
pub const OPTIONAL: [usize; 6] = [8, 9, 10, 11, 13, 17];
/// Spread, never mapped.
pub const SPREAD: [usize; 3] = [8, 9, 10];

/// A model's bone (its index) per NM part.
pub type Map = [Option<usize>; 21];

/// What a skeleton is to DMS.
pub struct Class {
    /// `ValveBiped`, a known rig's name, `by names` (guessed), `mapped` (the player's), `unknown`, `not humanoid`.
    pub rig: String,
    pub map: Map,
    /// Required parts without a bone.
    pub missing: Vec<&'static str>,
    /// Why the bones found can't be NM's joints (a leg not under its thigh, zero lengths ...).
    pub problems: Vec<String>,
}

impl Class {
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.problems.is_empty()
    }

    pub fn humanoid(&self) -> bool {
        self.rig != "not humanoid"
    }

    /// Why it can't be fitted, for the player.
    pub fn why(&self, model: &str) -> String {
        if !self.humanoid() {
            return format!("{model} isn't a humanoid: no pelvis, spine, two legs and two arms found (animals and robots without a humanoid chain aren't supported); if it is one, make its rig in the rig editor (dms-rig)");
        }
        let mut what: Vec<String> = self.missing.iter().map(|p| format!("no {p}")).collect();
        what.extend(self.problems.iter().cloned());
        format!("{model}: {} skeleton, {}: make its rig in the rig editor (dms-rig)", self.rig, what.join(", "))
    }
}

/// Known rigs: a bone name per part (`None`: not mapped, or the rig has none). Names compare without case, after any
/// prefix ending in `:` or `|` (Mixamo's `mixamorig:`, Blender's `Armature|`).
struct Rig {
    name: &'static str,
    bones: [Option<&'static str>; 21],
}

macro_rules! rig {
    ($name:literal, $pelvis:literal, [$lt:literal, $lc:literal, $lf:literal], [$rt:literal, $rc:literal, $rf:literal], $spine:literal, $neck:literal, $head:literal,
     [$lcl:literal, $lu:literal, $ll:literal, $lh:literal], [$rcl:literal, $ru:literal, $rl:literal, $rh:literal]) => {
        Rig {
            name: $name,
            bones: [
                Some($pelvis), Some($lt), Some($lc), Some($lf), Some($rt), Some($rc), Some($rf), Some($spine), None, None, None, Some($neck),
                Some($head), Some($lcl), Some($lu), Some($ll), Some($lh), Some($rcl), Some($ru), Some($rl), Some($rh),
            ],
        }
    };
}

const RIGS: &[Rig] = &[
    rig!("ValveBiped", "ValveBiped.Bip01_Pelvis", ["ValveBiped.Bip01_L_Thigh", "ValveBiped.Bip01_L_Calf", "ValveBiped.Bip01_L_Foot"],
        ["ValveBiped.Bip01_R_Thigh", "ValveBiped.Bip01_R_Calf", "ValveBiped.Bip01_R_Foot"], "ValveBiped.Bip01_Spine", "ValveBiped.Bip01_Neck1", "ValveBiped.Bip01_Head1",
        ["ValveBiped.Bip01_L_Clavicle", "ValveBiped.Bip01_L_UpperArm", "ValveBiped.Bip01_L_Forearm", "ValveBiped.Bip01_L_Hand"],
        ["ValveBiped.Bip01_R_Clavicle", "ValveBiped.Bip01_R_UpperArm", "ValveBiped.Bip01_R_Forearm", "ValveBiped.Bip01_R_Hand"]),
    rig!("Biped", "Bip01 Pelvis", ["Bip01 L Thigh", "Bip01 L Calf", "Bip01 L Foot"], ["Bip01 R Thigh", "Bip01 R Calf", "Bip01 R Foot"], "Bip01 Spine", "Bip01 Neck", "Bip01 Head",
        ["Bip01 L Clavicle", "Bip01 L UpperArm", "Bip01 L Forearm", "Bip01 L Hand"], ["Bip01 R Clavicle", "Bip01 R UpperArm", "Bip01 R Forearm", "Bip01 R Hand"]),
    rig!("Biped", "Bip001 Pelvis", ["Bip001 L Thigh", "Bip001 L Calf", "Bip001 L Foot"], ["Bip001 R Thigh", "Bip001 R Calf", "Bip001 R Foot"], "Bip001 Spine", "Bip001 Neck", "Bip001 Head",
        ["Bip001 L Clavicle", "Bip001 L UpperArm", "Bip001 L Forearm", "Bip001 L Hand"], ["Bip001 R Clavicle", "Bip001 R UpperArm", "Bip001 R Forearm", "Bip001 R Hand"]),
    rig!("Biped", "Bip01_Pelvis", ["Bip01_L_Thigh", "Bip01_L_Calf", "Bip01_L_Foot"], ["Bip01_R_Thigh", "Bip01_R_Calf", "Bip01_R_Foot"], "Bip01_Spine", "Bip01_Neck", "Bip01_Head",
        ["Bip01_L_Clavicle", "Bip01_L_UpperArm", "Bip01_L_Forearm", "Bip01_L_Hand"], ["Bip01_R_Clavicle", "Bip01_R_UpperArm", "Bip01_R_Forearm", "Bip01_R_Hand"]),
    rig!("Mixamo", "Hips", ["LeftUpLeg", "LeftLeg", "LeftFoot"], ["RightUpLeg", "RightLeg", "RightFoot"], "Spine", "Neck", "Head",
        ["LeftShoulder", "LeftArm", "LeftForeArm", "LeftHand"], ["RightShoulder", "RightArm", "RightForeArm", "RightHand"]),
    rig!("Unreal", "pelvis", ["thigh_l", "calf_l", "foot_l"], ["thigh_r", "calf_r", "foot_r"], "spine_01", "neck_01", "head",
        ["clavicle_l", "upperarm_l", "lowerarm_l", "hand_l"], ["clavicle_r", "upperarm_r", "lowerarm_r", "hand_r"]),
    rig!("TF2", "bip_pelvis", ["bip_hip_L", "bip_knee_L", "bip_foot_L"], ["bip_hip_R", "bip_knee_R", "bip_foot_R"], "bip_spine_0", "bip_neck", "bip_head",
        ["bip_collar_L", "bip_upperArm_L", "bip_lowerArm_L", "bip_hand_L"], ["bip_collar_R", "bip_upperArm_R", "bip_lowerArm_R", "bip_hand_R"]),
    rig!("VRoid", "J_Bip_C_Hips", ["J_Bip_L_UpperLeg", "J_Bip_L_LowerLeg", "J_Bip_L_Foot"], ["J_Bip_R_UpperLeg", "J_Bip_R_LowerLeg", "J_Bip_R_Foot"], "J_Bip_C_Spine", "J_Bip_C_Neck",
        "J_Bip_C_Head", ["J_Bip_L_Shoulder", "J_Bip_L_UpperArm", "J_Bip_L_LowerArm", "J_Bip_L_Hand"], ["J_Bip_R_Shoulder", "J_Bip_R_UpperArm", "J_Bip_R_LowerArm", "J_Bip_R_Hand"]),
    rig!("MMD", "下半身", ["左足", "左ひざ", "左足首"], ["右足", "右ひざ", "右足首"], "上半身", "首", "頭", ["左肩", "左腕", "左ひじ", "左手首"], ["右肩", "右腕", "右ひじ", "右手首"]),
];

/// A bone's name without a prefix ending in `:` or `|`.
fn bare(name: &str) -> &str {
    name.rsplit([':', '|']).next().unwrap_or(name)
}

fn find_named(s: &Skeleton, name: &str) -> Option<usize> {
    s.find(name).or_else(|| s.bones.iter().position(|b| bare(&b.name).eq_ignore_ascii_case(name)))
}

/// How many bones up to the root.
fn depth(s: &Skeleton, mut b: usize) -> usize {
    let mut d = 0;
    while let Some(p) = s.bones[b].parent {
        b = p;
        d += 1;
    }
    d
}

/// Whether `b` is somewhere under `a`.
pub fn under(s: &Skeleton, b: usize, a: usize) -> bool {
    let mut at = s.bones[b].parent;
    while let Some(p) = at {
        if p == a {
            return true;
        }
        at = s.bones[p].parent;
    }
    false
}

/// A name's words, lower case: split at anything not a letter or digit, at a lower-to-upper case change and between
/// letters and digits (`ValveBiped.Bip01_L_UpperArm` -> valve biped bip 01 l upper arm).
fn words(name: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut last: Option<char> = None;
    for c in bare(name).chars() {
        let split = match last {
            None => false,
            Some(l) => !c.is_alphanumeric() || (l.is_lowercase() && c.is_uppercase()) || (l.is_ascii_digit() != c.is_ascii_digit() && l.is_alphanumeric()),
        };
        if split && !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        }
        last = Some(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    L,
    R,
}

fn side(w: &[String]) -> Option<Side> {
    let has = |x: &str| w.iter().any(|t| t == x);
    match (has("l") || has("left") || w.iter().any(|t| t.starts_with("left")), has("r") || has("right") || w.iter().any(|t| t.starts_with("right"))) {
        (true, false) => Some(Side::L),
        (false, true) => Some(Side::R),
        _ => None,
    }
}

/// Bones no joint is: helpers, twists, IK targets, ends, attachments.
const NOT_JOINTS: &[&str] = &[
    "twist", "roll", "ik", "target", "pole", "helper", "end", "nub", "jiggle", "bend", "fix", "adj", "dummy", "ctrl", "control", "mch", "tgt", "wgt", "socket", "attach",
    "weapon", "prop", "lookat", "aim", "goal", "corrective", "deform", "physics",
];

/// A part's words: any of the patterns (all its words in the name), none of `not`.
struct Pattern {
    any: &'static [&'static [&'static str]],
    not: &'static [&'static str],
    side: bool,
}

const PELVIS: Pattern = Pattern { any: &[&["pelvis"], &["hips"], &["hip"]], not: &[], side: false };
const THIGH: Pattern = Pattern { any: &[&["thigh"], &["up", "leg"], &["upper", "leg"], &["upleg"], &["upperleg"], &["hip"]], not: &[], side: true };
const CALF: Pattern = Pattern { any: &[&["calf"], &["knee"], &["shin"], &["lower", "leg"], &["lowerleg"], &["leg"]], not: &["up", "upper", "thigh", "upleg", "upperleg"], side: true };
const FOOT: Pattern = Pattern { any: &[&["foot"], &["ankle"]], not: &["toe", "toes", "ball"], side: true };
const SPINE: Pattern = Pattern { any: &[&["spine"], &["waist"], &["abdomen"], &["torso"]], not: &[], side: false };
const NECK: Pattern = Pattern { any: &[&["neck"]], not: &[], side: false };
const HEAD: Pattern = Pattern { any: &[&["head"]], not: &["top", "front", "eye", "hair"], side: false };
const CLAVICLE: Pattern = Pattern { any: &[&["clavicle"], &["shoulder"], &["collar"], &["collarbone"]], not: &["pad", "armor", "armour"], side: true };
const UPPER_ARM: Pattern = Pattern { any: &[&["upper", "arm"], &["upperarm"], &["arm"]], not: &["fore", "lower", "forearm", "lowerarm"], side: true };
const FOREARM: Pattern = Pattern { any: &[&["forearm"], &["fore", "arm"], &["lower", "arm"], &["lowerarm"], &["elbow"]], not: &[], side: true };
const HAND: Pattern = Pattern { any: &[&["hand"], &["wrist"]], not: &["finger", "thumb", "index", "middle", "ring", "pinky", "little", "attach"], side: true };

impl Pattern {
    fn matches(&self, w: &[String], want: Option<Side>) -> bool {
        let has = |x: &str| w.iter().any(|t| t == x);
        if NOT_JOINTS.iter().chain(self.not).any(|x| has(x)) {
            return false;
        }
        // `hip` without a side is a pelvis, with one a thigh.
        if self.side != want.is_some() || side(w) != want {
            return false;
        }
        self.any.iter().any(|p| p.iter().all(|x| has(x)))
    }
}

/// The bone nearest the root matching `p`, under `within` if given (and not one already taken).
fn pick(s: &Skeleton, w: &[Vec<String>], p: &Pattern, want: Option<Side>, within: Option<usize>, taken: &[usize]) -> Option<usize> {
    (0..s.bones.len())
        .filter(|&b| !taken.contains(&b) && p.matches(&w[b], want) && within.is_none_or(|a| under(s, b, a)))
        .min_by_key(|&b| (depth(s, b), b))
}

/// NM's joints guessed from the bones' names and their hierarchy.
fn guess(s: &Skeleton) -> Map {
    let w: Vec<Vec<String>> = s.bones.iter().map(|b| words(&b.name)).collect();
    let mut m: Map = [None; 21];
    let mut taken: Vec<usize> = vec![];
    // Each part in turn, under the first of the parts given that's found; a limb's bone only under the one before it.
    let steps: [(usize, &Pattern, Option<Side>, &[usize], bool); 18] = [
        (0, &PELVIS, None, &[], false),
        (7, &SPINE, None, &[0], false),
        (11, &NECK, None, &[7], false),
        (12, &HEAD, None, &[11, 7], false),
        (1, &THIGH, Some(Side::L), &[], false),
        (2, &CALF, Some(Side::L), &[1], true),
        (3, &FOOT, Some(Side::L), &[2], true),
        (4, &THIGH, Some(Side::R), &[], false),
        (5, &CALF, Some(Side::R), &[4], true),
        (6, &FOOT, Some(Side::R), &[5], true),
        (13, &CLAVICLE, Some(Side::L), &[7], false),
        (14, &UPPER_ARM, Some(Side::L), &[13, 7], false),
        (15, &FOREARM, Some(Side::L), &[14], true),
        (16, &HAND, Some(Side::L), &[15], true),
        (17, &CLAVICLE, Some(Side::R), &[7], false),
        (18, &UPPER_ARM, Some(Side::R), &[17, 7], false),
        (19, &FOREARM, Some(Side::R), &[18], true),
        (20, &HAND, Some(Side::R), &[19], true),
    ];
    for (k, p, side, within, strict) in steps {
        let parent = within.iter().find_map(|&a| m[a]);
        if strict && parent.is_none() {
            continue;
        }
        // Under its parent part if one is there, else anywhere.
        m[k] = parent.and_then(|a| pick(s, &w, p, side, Some(a), &taken)).or_else(|| if strict { None } else { pick(s, &w, p, side, None, &taken) });
        taken.extend(m[k]);
    }
    m
}

/// A model's bone per part from its names (`{part = bone}`, the mapping editor's and `data/dmr/skeletons`'): unknown
/// parts or bones are an error; Spine1 to Spine3 are ignored (spread).
pub fn from_names(s: &Skeleton, names: &[(String, String)]) -> Result<Map, String> {
    let mut m: Map = [None; 21];
    for (part, bone) in names {
        let k = PARTS.iter().position(|p| p.eq_ignore_ascii_case(part)).ok_or_else(|| format!("no NM part {part}"))?;
        if SPREAD.contains(&k) || bone.is_empty() {
            continue;
        }
        m[k] = Some(s.find(bone).ok_or_else(|| format!("{part}: the model has no bone {bone}"))?);
    }
    Ok(m)
}

/// Why `m` can't be NM's joints on `s`: required parts without a bone, and problems with the bones it has.
pub fn check(s: &Skeleton, m: &Map, at: &[Vec3]) -> (Vec<&'static str>, Vec<String>) {
    let missing: Vec<&'static str> = (0..21).filter(|k| !OPTIONAL.contains(k) && m[*k].is_none()).map(|k| PARTS[k]).collect();
    let mut problems = vec![];
    let mut seen: Vec<usize> = vec![];
    for k in 0..21 {
        if let Some(b) = m[k] {
            if seen.contains(&b) {
                problems.push(format!("{} is {} again", PARTS[k], s.bones[b].name));
            }
            seen.push(b);
        }
    }
    // Each limb's bone under the one before it, the head and neck under the spine, a length to each segment.
    let chains: [&[usize]; 5] = [&[1, 2, 3], &[4, 5, 6], &[7, 11, 12], &[13, 14, 15, 16], &[17, 18, 19, 20]];
    for chain in chains {
        let bones: Vec<(usize, usize)> = chain.iter().filter_map(|&k| m[k].map(|b| (k, b))).collect();
        for pair in bones.windows(2) {
            let ((ka, a), (kb, b)) = (pair[0], pair[1]);
            if !under(s, b, a) {
                problems.push(format!("{} ({}) isn't under {} ({})", PARTS[kb], s.bones[b].name, PARTS[ka], s.bones[a].name));
            } else if at[a].distance(at[b]) * M < 0.02 {
                problems.push(format!("{} and {} are in one place", PARTS[ka], PARTS[kb]));
            }
        }
    }
    if let (Some(l), Some(r)) = (m[1], m[4])
        && at[l].distance(at[r]) * M < 0.02
    {
        problems.push("the thighs are in one place".into());
    }
    if let (Some(p), Some(h)) = (m[0], m[12])
        && at[p].distance(at[h]) * M < 0.15
    {
        problems.push("the head is at the pelvis".into());
    }
    (missing, problems)
}

/// A skeleton's class: a known rig, a guess by names, or a player's mapping (`names`); `at` the bones' positions
/// (units, the model's space) the fit will use.
pub fn classify(s: &Skeleton, names: Option<&[(String, String)]>, at: &[Vec3]) -> Result<Class, String> {
    if let Some(names) = names {
        let map = from_names(s, names)?;
        let (missing, problems) = check(s, &map, at);
        return Ok(Class { rig: "mapped".into(), map, missing, problems });
    }
    for rig in RIGS {
        let mut map: Map = [None; 21];
        for k in 0..21 {
            map[k] = rig.bones[k].and_then(|n| find_named(s, n));
        }
        let (missing, problems) = check(s, &map, at);
        if missing.is_empty() && problems.is_empty() {
            return Ok(Class { rig: rig.name.into(), map, missing, problems });
        }
    }
    let map = guess(s);
    let (missing, problems) = check(s, &map, at);
    let found = |ks: &[usize]| ks.iter().all(|&k| map[k].is_some());
    let rig = if missing.is_empty() && problems.is_empty() {
        "by names"
    } else if !found(&[1, 4, 14, 18]) || (map[0].is_none() && map[7].is_none()) {
        "not humanoid"
    } else {
        "unknown"
    };
    Ok(Class { rig: rig.into(), map, missing, problems })
}

/// The bones strictly between the spine's and the neck's (or the head's), from the spine up: NM's Spine1 to Spine3
/// are spread over them.
pub fn spine_chain(s: &Skeleton, m: &Map) -> Vec<usize> {
    let (Some(spine), Some(top)) = (m[7], m[11].or(m[12])) else { return vec![] };
    let mut out = vec![];
    let mut at = s.bones[top].parent;
    while let Some(b) = at {
        if b == spine {
            out.reverse();
            return out;
        }
        out.push(b);
        at = s.bones[b].parent;
    }
    vec![]
}

/// The player's edits of a model's fit (the mapping editor, docs/adr/0030): where NM's joints are against their bones
/// (the fit's input, as the spine's spread), and the parts' sizes, masses and how they lie on the model's bones. None by
/// default; Homer and the golden runs never have them.
#[derive(Clone, PartialEq, Default)]
pub struct Edits {
    /// Each joint moved from where its bone put it (units, the model's space, its own pose).
    pub joints: [Vec3; 21],
    pub parts: [PartEdit; 21],
}

/// One part's edits: its capsule's radius and half length or its box's half extents (m), its mass (kg), and where it lies
/// on the model's bone: moved (m, the part's frame) and turned (pitch, yaw, roll; degrees).
#[derive(Clone, Copy, PartialEq, Default)]
pub struct PartEdit {
    pub radius: Option<f32>,
    pub half: Option<f32>,
    pub boxed: Option<Vec3>,
    pub mass: Option<f32>,
    pub offset: Vec3,
    pub turn: Vec3,
}

impl PartEdit {
    /// The part's placement on the model's bone: moved, then turned.
    pub fn adjust(&self) -> Option<Transform> {
        if self.offset == Vec3::ZERO && self.turn == Vec3::ZERO {
            return None;
        }
        let [p, y, r] = self.turn.to_array().map(f32::to_radians);
        // Source's angles: yaw about Z, pitch about Y, roll about X.
        let rot = Quat::from_rotation_z(y) * Quat::from_rotation_y(p) * Quat::from_rotation_x(r);
        Some(Transform::from_translation(self.offset).with_rotation(rot.normalize()))
    }
}

/// A build's part sizes as edited: a capsule's radius and half length or a box's extents, its inertia scaled as the
/// shape's own moments are.
pub fn apply_sizes(build: &mut delirium::ragdoll::Build, e: &Edits) {
    // A shape's moments per unit mass, along the part's axes (`PartDef::moments`' formulas).
    let moments = |p: &delirium::ragdoll::PartDef| match p.boxed {
        Some(h) => Vec3::new(h.y * h.y + h.z * h.z, h.x * h.x + h.z * h.z, h.x * h.x + h.y * h.y) / 3.0,
        None => {
            let (r, l) = (p.radius, 2.0 * (p.half + p.radius));
            Vec3::new((3.0 * r * r + l * l) / 12.0, 0.5 * r * r, (3.0 * r * r + l * l) / 12.0)
        }
    };
    for (p, pe) in build.parts.iter_mut().zip(&e.parts) {
        let before = moments(p);
        match p.boxed {
            Some(_) => {
                if let Some(h) = pe.boxed.filter(|h| h.min_element() > 0.0) {
                    p.boxed = Some(h);
                    p.radius = h.min_element();
                }
            }
            None => {
                p.radius = pe.radius.filter(|&r| r > 0.0).unwrap_or(p.radius);
                p.half = pe.half.filter(|&h| h >= 0.0).unwrap_or(p.half);
            }
        }
        let after = moments(p);
        if p.inertia.min_element() > 0.0 && before.min_element() > 0.0 {
            p.inertia *= after / before;
        }
    }
}

/// The parts' masses as edited: each `heft` scaled so NM weighs the part `mass` (`now` its masses as NM has them).
pub fn apply_masses(build: &mut delirium::ragdoll::Build, e: &Edits, now: &[f32]) {
    for ((p, pe), &m) in build.parts.iter_mut().zip(&e.parts).zip(now) {
        if let Some(want) = pe.mass.filter(|&w| w > 0.0) {
            p.heft *= want / m.max(1e-3);
        }
    }
}

impl Edits {
    pub fn sizes(&self) -> bool {
        self.parts.iter().any(|p| p.radius.is_some() || p.half.is_some() || p.boxed.is_some())
    }

    pub fn masses(&self) -> bool {
        self.parts.iter().any(|p| p.mass.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split() {
        assert_eq!(words("ValveBiped.Bip01_L_UpperArm"), ["valve", "biped", "bip", "01", "l", "upper", "arm"]);
        assert_eq!(words("mixamorig:LeftUpLeg"), ["left", "up", "leg"]);
        assert_eq!(words("thigh_twist_01_l"), ["thigh", "twist", "01", "l"]);
        assert_eq!(words("bip_upperArm_R"), ["bip", "upper", "arm", "r"]);
    }

    #[test]
    fn patterns() {
        let w = |n: &str| words(n);
        assert!(THIGH.matches(&w("bip_hip_L"), Some(Side::L)));
        assert!(PELVIS.matches(&w("Hips"), None));
        assert!(!PELVIS.matches(&w("bip_hip_L"), None));
        assert!(CALF.matches(&w("LeftLeg"), Some(Side::L)));
        assert!(!CALF.matches(&w("LeftUpLeg"), Some(Side::L)));
        assert!(UPPER_ARM.matches(&w("RightArm"), Some(Side::R)));
        assert!(!UPPER_ARM.matches(&w("RightForeArm"), Some(Side::R)));
        assert!(!HAND.matches(&w("LeftHandIndex1"), Some(Side::L)));
        assert!(!THIGH.matches(&w("thigh_twist_01_l"), Some(Side::L)));
    }
}
