//! Homer's build fitted to any humanoid model (ROADMAP M9–M12, docs/adr/0025, 0027, 0030): the model's skeleton read
//! from its `.mdl`, its rig found by the bones' names or made by the player (`rigs`), and the fit (`fit`). Shared by
//! the module (`gmsv_dmr`) and the rig editor (`dms-rig`), so a rig fits the same in both.

pub mod fit;
pub mod rigs;
pub mod skeleton;

/// Metres per Source unit.
pub const M: f32 = 0.0254;