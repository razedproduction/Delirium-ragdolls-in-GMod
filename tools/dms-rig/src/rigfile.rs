//! A model's rig file, `garrysmod/data/dmr/skeletons/<model>.json` (format 2, docs/adr/0030): what the addon reads
//! (`addon/lua/dmr/skeletons.lua`) and reloads as it changes. `{dms_skeleton = 2, model, checksum, bones = {[part] =
//! bone}, joints = {[part] = {x, y, z}} (units, the model's space), parts = {[part] = {radius, half, box = {x, y, z}
//! (m), mass (kg), offset = {x, y, z} (m, the part's frame), turn = {p, y, r} (degrees)}}}`.

use delirium::prelude::*;
use dms_fit::rigs::{self, Edits, PARTS};
use serde_json::{Map, Value, json};

pub const FORMAT: i64 = 2;

/// A rig as a file holds it.
#[derive(Clone, Default, PartialEq)]
pub struct Rig {
    /// The player's mapping (`{part, bone}`; empty: the model's own rig).
    pub bones: Vec<(String, String)>,
    pub edits: Edits,
    pub model: Option<String>,
    pub checksum: Option<i32>,
}

impl Rig {
    /// The fit's arguments, as the addon's `DMR_RigArgs` makes them: the mapping (none if empty) and the edits (none
    /// if nothing is edited).
    pub fn args(&self) -> (Option<&[(String, String)]>, Option<&Edits>) {
        let edited = self.edits.joints.iter().any(|j| *j != Vec3::ZERO) || self.edits.parts.iter().any(|p| *p != rigs::PartEdit::default());
        ((!self.bones.is_empty()).then_some(self.bones.as_slice()), edited.then_some(&self.edits))
    }
}

/// The rig file's path under `data/dmr/skeletons/` for a model (`skeletons.lua`'s `path`).
pub fn file_name(model: &str) -> String {
    // Lua's string functions go byte by byte: a UTF-8 letter is as many `_`.
    let m = model.to_ascii_lowercase().replace('\\', "/");
    let m = m.strip_prefix("models/").unwrap_or(&m);
    let m = m.strip_suffix(".mdl").unwrap_or(m);
    let clean: String = m.bytes().map(|c| if c.is_ascii_alphanumeric() || b"_-/.".contains(&c) { c as char } else { '_' }).collect();
    format!("{clean}.json")
}

fn vec3(v: &Value) -> Option<Vec3> {
    let n = |x: Option<&Value>| x.and_then(Value::as_f64).map(|f| f as f32);
    let (x, y, z) = match v {
        Value::Array(a) => (n(a.first()), n(a.get(1)), n(a.get(2))),
        Value::Object(o) => (n(o.get("x")), n(o.get("y")), n(o.get("z"))),
        _ => return None,
    };
    Some(Vec3::new(x?, y?, z?)).filter(|v| v.is_finite())
}

fn part(name: &str) -> Option<usize> {
    PARTS.iter().position(|p| p.eq_ignore_ascii_case(name))
}

/// A rig from a file's text (format 1 had bones only), made safe as `skeletons.lua`'s `clean` does: unknown parts and
/// bad numbers left out.
pub fn decode(text: &str) -> Result<Rig, String> {
    let t: Value = serde_json::from_str(text).map_err(|e| format!("not JSON: {e}"))?;
    let bones = t.get("bones").and_then(Value::as_object).ok_or("not a DMS rig (no bones)")?;
    if t.get("dms_skeleton").and_then(Value::as_i64).unwrap_or(0) > FORMAT {
        return Err(format!("a rig from a newer DMS (format {})", t["dms_skeleton"]));
    }
    let mut rig = Rig { model: t.get("model").and_then(Value::as_str).map(str::to_string), checksum: t.get("checksum").and_then(Value::as_i64).map(|c| c as i32), ..Rig::default() };
    for p in PARTS {
        if let Some(b) = bones.iter().find(|(k, _)| k.eq_ignore_ascii_case(p)).and_then(|(_, v)| v.as_str()).filter(|b| !b.is_empty()) {
            rig.bones.push((p.to_string(), b.to_string()));
        }
    }
    if let Some(j) = t.get("joints").and_then(Value::as_object) {
        for (k, v) in j {
            if let (Some(k), Some(v)) = (part(k), vec3(v)) {
                rig.edits.joints[k] = v;
            }
        }
    }
    if let Some(ps) = t.get("parts").and_then(Value::as_object) {
        for (k, v) in ps {
            let (Some(k), Some(o)) = (part(k), v.as_object()) else { continue };
            let num = |n: &str| o.get(n).and_then(Value::as_f64).map(|f| f as f32).filter(|f| f.is_finite());
            rig.edits.parts[k] = rigs::PartEdit {
                radius: num("radius"),
                half: num("half"),
                boxed: o.get("box").and_then(vec3),
                mass: num("mass"),
                offset: o.get("offset").and_then(vec3).unwrap_or(Vec3::ZERO),
                turn: o.get("turn").and_then(vec3).unwrap_or(Vec3::ZERO),
            };
        }
    }
    Ok(rig)
}

fn arr(v: Vec3) -> Value {
    // Rounded: a rig is text to read and share; a millionth of a unit or a metre changes nothing.
    let r = |x: f32| (f64::from(x) * 1e6).round() / 1e6;
    json!([r(v.x), r(v.y), r(v.z)])
}

/// The rig's file text: only what changes the fit, parts in NM's order.
pub fn encode(rig: &Rig) -> String {
    let mut t = Map::new();
    t.insert("dms_skeleton".into(), json!(FORMAT));
    if let Some(m) = &rig.model {
        t.insert("model".into(), json!(m));
    }
    if let Some(c) = rig.checksum {
        t.insert("checksum".into(), json!(c));
    }
    let mut bones = Map::new();
    for (p, b) in &rig.bones {
        bones.insert(p.clone(), json!(b));
    }
    t.insert("bones".into(), Value::Object(bones));
    let mut joints = Map::new();
    for (k, j) in rig.edits.joints.iter().enumerate() {
        if *j != Vec3::ZERO {
            joints.insert(PARTS[k].into(), arr(*j));
        }
    }
    t.insert("joints".into(), Value::Object(joints));
    let mut parts = Map::new();
    let r = |x: f32| (f64::from(x) * 1e6).round() / 1e6;
    for (k, p) in rig.edits.parts.iter().enumerate() {
        let mut o = Map::new();
        if let Some(v) = p.radius {
            o.insert("radius".into(), json!(r(v)));
        }
        if let Some(v) = p.half {
            o.insert("half".into(), json!(r(v)));
        }
        if let Some(v) = p.boxed {
            o.insert("box".into(), arr(v));
        }
        if let Some(v) = p.mass {
            o.insert("mass".into(), json!(r(v)));
        }
        if p.offset != Vec3::ZERO {
            o.insert("offset".into(), arr(p.offset));
        }
        if p.turn != Vec3::ZERO {
            o.insert("turn".into(), arr(p.turn));
        }
        if !o.is_empty() {
            parts.insert(PARTS[k].into(), Value::Object(o));
        }
    }
    t.insert("parts".into(), Value::Object(parts));
    serde_json::to_string_pretty(&Value::Object(t)).unwrap_or_default() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut rig = Rig { model: Some("models/player/x.mdl".into()), checksum: Some(-5), ..Rig::default() };
        rig.bones.push((PARTS[0].into(), "Hips".into()));
        rig.edits.joints[3] = Vec3::new(1.0, -2.5, 0.25);
        rig.edits.parts[12].radius = Some(0.11);
        rig.edits.parts[0].boxed = Some(Vec3::new(0.1, 0.2, 0.3));
        rig.edits.parts[16].turn = Vec3::new(10.0, 0.0, -5.0);
        let back = decode(&encode(&rig)).unwrap();
        assert!(back == rig);
    }

    #[test]
    fn names() {
        assert_eq!(file_name("models/Player/Some Guy.mdl"), "player/some_guy.json");
        assert_eq!(file_name("models\\x.mdl"), "x.json");
    }
}
