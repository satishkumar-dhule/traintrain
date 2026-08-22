//! Fixture-driven integration tests for the askdisha (Corover/IRCTC Ask DISHA)
//! module. Parses real captured upstream payloads from `testdata/askdisha/`
//! through the contract structs in `src/core/corover.rs`.
//!
//! Contract: `docs/ASKDISHA_MODULE.md` (Fixtures + Rust types sections).

use railway_rs::core::corover::{
    NearbyStation, PinLookup, ScheduleResponse, SettingsFlag, StationRow,
};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!("testdata/askdisha/{name}")).unwrap_or_else(|e| {
        panic!("missing fixture testdata/askdisha/{name}: {e}");
    })
}

#[test]
fn schedule_12951_parses_through_contract_struct() {
    let raw = fixture("schedule_12951.json");
    let sched: ScheduleResponse = serde_json::from_str(&raw).expect("schedule must deserialize");

    assert_eq!(sched.train_number, "12951");
    assert_eq!(sched.train_name.as_deref(), Some("NDLS TEJAS RAJ"));
    assert_eq!(sched.station_from.as_deref(), Some("MMCT"));
    assert_eq!(sched.station_to.as_deref(), Some("NDLS"));
    assert!(
        !sched.station_list.is_empty(),
        "station_list must be non-empty"
    );

    // Upstream sends "Y"/"N" strings; the struct must expose plain bools.
    let runs: [bool; 7] = [
        sched.runs_mon,
        sched.runs_tue,
        sched.runs_wed,
        sched.runs_thu,
        sched.runs_fri,
        sched.runs_sat,
        sched.runs_sun,
    ];
    assert!(
        runs.iter().all(|v| *v),
        "12951 is a daily train, all run flags true"
    );

    let first = &sched.station_list[0];
    assert_eq!(first.station_code, "MMCT");
    assert_eq!(first.station_name, "MUMBAI CENTRAL");
    assert_eq!(first.departure_time.as_deref(), Some("17:00"));
    let last = sched.station_list.last().unwrap();
    assert_eq!(last.station_code, "NDLS");
    assert_eq!(last.arrival_time.as_deref(), Some("08:32"));
}

#[test]
fn stations_new_parses_through_contract_struct() {
    let raw = fixture("stations_new.json");
    let stations: Vec<StationRow> = serde_json::from_str(&raw).expect("stations must deserialize");

    assert!(!stations.is_empty(), "station_list must be non-empty");
    let first = &stations[0];
    assert_eq!(first.code, "NEW");
    assert!(!first.name.is_empty());
    assert!(first.latitude.is_some() && first.longitude.is_some());
}

#[test]
fn get_settings_parses_through_contract_struct() {
    let raw = fixture("getSettings.json");
    let settings: SettingsFlag = serde_json::from_str(&raw).expect("settings must deserialize");

    assert_eq!(settings.id, 1);
    assert!(!settings.is_disabled);
    assert!(settings.booking);
}

#[test]
fn faqs_en_is_valid_string_array_with_at_least_50_entries() {
    let faqs: Vec<String> =
        serde_json::from_str(&fixture("faqs_en.json")).expect("faqs must be a JSON string array");
    assert!(
        faqs.len() > 50,
        "expected >=50 truncated FAQ entries, got {}",
        faqs.len()
    );
    assert!(faqs.iter().all(|f| !f.trim().is_empty()));
}

#[test]
fn nearby_mumbai_parses_through_contract_struct() {
    let raw = fixture("nearby_mumbai.json");
    let rows: Vec<NearbyStation> = serde_json::from_str(&raw).expect("nearby must deserialize");

    assert!(!rows.is_empty(), "nearby rows must be non-empty");
    // Upstream carries real km distances; they must survive parsing as f64.
    assert!(
        rows.iter()
            .all(|r| r.distance.is_some_and(|d| d.is_finite())),
        "every captured row has a finite distance"
    );
    // Fixture is captured already nearest-first.
    assert!(
        rows.windows(2)
            .all(|w| w[0].distance.unwrap_or(f64::INFINITY)
                <= w[1].distance.unwrap_or(f64::INFINITY)),
        "capture is distance-sorted"
    );

    let first = &rows[0];
    assert_eq!(first.code, "CLA");
    assert_eq!(first.name, "KURLA JN");
    assert_eq!(first.name_hi.as_deref(), Some("कुर्ला जन"));
    assert_eq!(first.name_gu.as_deref(), Some("કુર્લા જન"));
    assert_eq!(first.district.as_deref(), Some("Mumbai Suburban"));
    assert_eq!(first.state.as_deref(), Some("Maharashtra"));

    // Unknown upstream fields (trainCount/latitude/longitude/address) are
    // ignored by the contract struct - the parse above tolerates them.

    // The fixture holds fewer rows than the 50-row response cap.
    assert!(rows.len() < 50);
}

#[test]
fn pin_400001_parses_through_contract_struct() {
    let raw = fixture("pin_400001.json");
    let pin: PinLookup = serde_json::from_str(&raw).expect("pin must deserialize");

    assert_eq!(pin.state, "MAHARASHTRA");
    assert_eq!(pin.city_list, vec!["Raigarh(MH)", "Mumbai"]);
}

#[test]
fn unauthorized_body_has_status_401() {
    let v: serde_json::Value =
        serde_json::from_str(&fixture("unauthorized.json")).expect("unauthorized must be JSON");
    assert_eq!(v["status"], 401);
    assert_eq!(v["message"], "Not Allowed!");
}
