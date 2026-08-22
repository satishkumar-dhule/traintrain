//! F1 offline hydration: merge the AskDISHA CDN `stationupdated.json` capture
//! into the shipped `data/stations.json`, keyed by station code.
//!
//! Inputs (read-only):
//! - `testdata/askdisha/stationupdated_full.json` - 8,491 upstream rows with
//!   optional `name_hi`/`name_gu`/`district`/`address`/`trainCount` and
//!   `latitude`/`longitude`
//! - `data/stations.json` - the local NTES master dataset (authority)
//!
//! Output: `data/stations.json` rewritten in place with the optional
//! hydration fields added to matching records. Local `state`/`zone` (and
//! `code`/`name`) always win on conflict; blank upstream strings and
//! unusable coordinates become absent fields. The merge is deterministic,
//! so re-running over its own output is a byte-level no-op.
//!
//! Usage: `cargo run --bin hydrate_stations`

use std::path::Path;

use railway_rs::data::{load_stations, merge_hydration, HydrationRow, StationRecord};

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = root.join("testdata/askdisha/stationupdated_full.json");
    let out_path = root.join("data/stations.json");

    let local: Vec<StationRecord> = load_stations(&out_path)
        .unwrap_or_else(|e| die(format!("load {}: {e}", out_path.display())));
    let raw =
        std::fs::read(&fixture).unwrap_or_else(|e| die(format!("read {}: {e}", fixture.display())));
    let upstream: Vec<HydrationRow> = serde_json::from_slice(&raw)
        .unwrap_or_else(|e| die(format!("parse {}: {e}", fixture.display())));

    let (records, report) = merge_hydration(local, &upstream);

    let mut bytes = match serde_json::to_vec(&records) {
        Ok(bytes) => bytes,
        Err(e) => die(format!("serialize stations: {e}")),
    };
    bytes.push(b'\n');
    std::fs::write(&out_path, &bytes)
        .unwrap_or_else(|e| die(format!("write {}: {e}", out_path.display())));

    println!(
        "hydrated {}/{} codes, {} unmatched",
        report.hydrated,
        report.total,
        report.unmatched.len()
    );
    if !report.unmatched.is_empty() {
        eprintln!("unmatched upstream codes: {}", report.unmatched.join(", "));
    }
}

fn die(msg: String) -> ! {
    eprintln!("hydrate_stations: {msg}");
    std::process::exit(1);
}
