//! dmr-ctl: this repository's side of the bench (docs/bench.md). The channel, RCON, scenarios and records are the
//! golden checkout's `dms-ctl` library (docs/adr/0036); only where the bench's addresses live is this repository's:
//! `.bench/server.txt` here, written by `scripts\server.ps1 start`, never the golden checkout's.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub use dms_ctl::{Bench, Channel, Scenario, parse_duration, rcon, rec, run};

/// This repository (tools/dmr-ctl -> ..).
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().and_then(Path::parent).expect("the repository").to_path_buf()
}

/// The bench server's addresses and secrets from this repository's `.bench/server.txt` (`key=value` lines).
pub fn bench() -> Result<Bench, String> {
    let path = repo().join(".bench").join("server.txt");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e} (start the bench server: scripts\\server.ps1 start)", path.display()))?;
    let kv: HashMap<&str, &str> = text.trim_start_matches('\u{feff}').lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim(), v.trim())).collect();
    let get = |k: &str| kv.get(k).map(|v| v.to_string()).unwrap_or_default();
    Ok(Bench {
        dir: get("dir"),
        pid: get("pid"),
        rcon_port: get("rcon_port").parse().unwrap_or(27015),
        rcon_password: get("rcon_password"),
        dev_port: get("dev_port").parse().unwrap_or(27098),
        dev_token: get("dev_token"),
        log: get("log"),
    })
}
