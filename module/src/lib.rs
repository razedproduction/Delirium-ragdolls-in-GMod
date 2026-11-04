//! gmsv_dmr: Delirium (NM, FOLLOW) on GMod's own ragdolls, NPCs and Workshop models, on VPhysics-Box3D's
//! `IPhysicsNM` (the golden checkout's fork, docs/adr/0036).
//!
//! `require("dmr")` registers the global table `DMR`.

mod agents;
mod bench;
mod data;
mod dev;
mod ffi;
mod follow;
mod lua;
mod natives;
mod nm;
mod nm_natives;
mod ragdolls;
mod react;
mod selftest;
mod step;
mod tuning;
mod world;

use std::ffi::{CString, c_int};

use ffi::vphysics_nm::{self, AcquireError, Version};
use lua::{Api, LuaState};

const MODULE_VERSION: &str = env!("CARGO_PKG_VERSION");
/// The Delirium release built in: the SDK checkout's tag (build.rs).
const DELIRIUM_RELEASE: &str = env!("DMR_DELIRIUM_RELEASE");

// Only the x86-64 branch of GMod is built for.
const ARCH: &str = "win64";

/// The SDK's version (`major * 10000 + minor * 100 + patch`) as text.
fn sdk_version() -> String {
    let v = delirium_physics::nm_version();
    format!("{}.{}.{}", v / 10000, v / 100 % 100, v % 100)
}

lua::native!(lua_nm_version => nm_version_impl);
/// `DMR.NmVersion()`: the Delirium SDK's version number, as `nm_version` gives it.
unsafe fn nm_version_impl(api: &Api, l: *mut LuaState) -> Result<c_int, String> {
    unsafe { (api.pushnumber)(l, f64::from(delirium_physics::nm_version())) };
    Ok(1)
}

/// Looks for `IPhysicsNM` and says in the console what came of it (docs/adr/0005).
fn acquire_message() -> String {
    match vphysics_nm::acquire() {
        Ok(Version::V006) => format!("[DMR] IPhysicsNM {} found", Version::V006.name()),
        Ok(v) => format!(
            "[DMR] WARNING: only IPhysicsNM {} found: GMod's ragdolls need {} (native bodies), update VPhysics-Box3D",
            v.name(),
            Version::V006.name()
        ),
        Err(AcquireError::NoVphysics) => {
            "[DMR] ERROR: vphysics.dll isn't loaded, DMR is disabled".into()
        }
        Err(AcquireError::NotBox3d) => "[DMR] ERROR: the installed vphysics.dll isn't VPhysics-Box3D (no IPhysicsNM), \
                                        DMR is disabled: reinstall VPhysics-Box3D into garrysmod/bin/win64 (Steam's \
                                        file verification puts the original back)"
            .into(),
    }
}

/// # Safety
/// Called by GMod with its live server Lua state.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gmod13_open(l: *mut LuaState) -> c_int {
    // A panic here would abort the game (docs/adr/0021): the module stays inert instead.
    // SAFETY: the caller's promise.
    std::panic::catch_unwind(|| unsafe { open(l) }).unwrap_or(0)
}

unsafe fn open(l: *mut LuaState) -> c_int {
    // Without lua_shared there's no way to report anything: the module just stays inert.
    let Ok(api) = lua::api() else { return 0 };
    let nm_message = acquire_message();
    unsafe {
        let natives = natives::NATIVES.iter().chain(nm_natives::NATIVES).chain(agents::NATIVES).chain(dev::NATIVES).chain(world::NATIVES).chain(react::NATIVES).chain(ragdolls::NATIVES);
        (api.createtable)(l, 0, 3 + natives.clone().count() as c_int);
        let version = CString::new(MODULE_VERSION).unwrap_or_default();
        (api.pushstring)(l, version.as_ptr());
        (api.setfield)(l, -2, c"Version".as_ptr());
        // What the Lua addon checks at start (dmr_init.lua): the Delirium built in.
        let delirium = CString::new(DELIRIUM_RELEASE).unwrap_or_default();
        (api.pushstring)(l, delirium.as_ptr());
        (api.setfield)(l, -2, c"DeliriumVersion".as_ptr());
        (api.pushcclosure)(l, lua_nm_version, 0);
        (api.setfield)(l, -2, c"NmVersion".as_ptr());
        for (name, f) in natives {
            (api.pushcclosure)(l, *f, 0);
            (api.setfield)(l, -2, name.as_ptr());
        }
        (api.setfield)(l, lua::GLOBALSINDEX, c"DMR".as_ptr());

        let banner = format!("[DMR] gmsv_dmr {MODULE_VERSION} ({ARCH}), Delirium {DELIRIUM_RELEASE} (SDK {})", sdk_version());
        api.print(l, &CString::new(banner).unwrap_or_default());
        api.print(l, &CString::new(nm_message).unwrap_or_default());
    }
    0
}

/// # Safety
/// Called by GMod with the Lua state given to `gmod13_open`, as it shuts it down.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gmod13_close(l: *mut LuaState) -> c_int {
    // Each part on its own: the step callback must be off before the module goes, whatever else fails.
    let _ = std::panic::catch_unwind(dev::stop);
    let _ = std::panic::catch_unwind(step::shutdown);
    vphysics_nm::release();
    // GMod unloads the module with each map, and what the statics hold isn't freed with it (docs/adr/0021): ~65 MB
    // a map with agents (NM's data, the map read, its worlds).
    let _ = std::panic::catch_unwind(|| {
        data::unload();
        ragdolls::unload();
        world::unload();
        tuning::unload();
        *delirium::ragdoll::SURFACES.write().unwrap_or_else(|e| e.into_inner()) = Vec::new();
    });
    if let Ok(api) = lua::api() {
        unsafe {
            (api.pushnil)(l);
            (api.setfield)(l, lua::GLOBALSINDEX, c"DMR".as_ptr());
        }
    }
    0
}
