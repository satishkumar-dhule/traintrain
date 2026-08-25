mod common;

use common::TestApp;

/// Trimmed TrnMap route page: the `myStns` / `myStnsF` / `myStnNames` / `train`
/// / `runInfo` JavaScript blocks the route-map parser reads. Three halts
/// (NDLS -> GZB -> DDN), a six-code track polyline.
const ROUTE_HTML: &str = r##"<html><head><title>Train on Map</title></head><body><script>
var myStns = ["NDLS","GZB","DDN"];
var myStnsF = ["NDLS","CSB","TKJ","GZB","MTC","DDN"];
var myStnNames = ["NEW DELHI","GHAZIABAD","DEHRADOON"];
var train = ["12055","DDN JANSHTBDI","NEW DELHI","DEHRADOON","NDLS","DDN",""];
var runInfo = ["#15:20#1#0#Daily#Daily","15:53#15:55#1#26#Daily#Daily","16:55#17:35#1#219#Daily#Daily"];
</script></body></html>"##;

/// The same page in spot mode: `cStn` (current station) points at GZB, `jStn`
/// describes the queried journey station NDLS, and `runInfo` carries the
/// `arrTime|arrDelay#depTime|depDelay` pairs (Source/Destination terminals
/// report an "On Time" badge on the empty side).
const SPOT_HTML: &str = r##"<html><head><title>Train on Map</title></head><body><script>
var myStns = ["NDLS","GZB","DDN"];
var myStnsF = ["NDLS","CSB","TKJ","GZB","MTC","DDN"];
var myStnNames = ["NEW DELHI","GHAZIABAD","DEHRADOON"];
var train = ["12055","DDN JANSHTBDI","NEW DELHI","DEHRADOON","NDLS","DDN","17-Aug-2026"];
var cStn = ["GZB","--","--"];
var jStn = ["NDLS","New Delhi","","","<span class=blueS11L>Source</span>","17-Aug-2026 15:20","17-Aug-2026 15:20","On Time","9"];
var runInfo = ["Source|On Time#17-Aug-2026 15:20|On Time","17-Aug-2026 15:53|On Time#17-Aug-2026 15:55|On Time","Destination|On Time#17-Aug-2026 17:35|On Time"];
</script></body></html>"##;

#[tokio::test]
async fn missing_or_invalid_train_is_bad_request() {
    let app = TestApp::spawn().await;
    for path in [
        "/rail-api/ntes/train-on-map",
        "/rail-api/ntes/train-on-map?train=",
        "/rail-api/ntes/train-on-map?train=abc",
        "/rail-api/ntes/train-on-map?train=00000",
        "/rail-api/ntes/train-on-map?train=1234",
    ] {
        let (status, _) = app.get(path).await;
        assert_eq!(status, 400, "path {path} should be 400");
    }
}

#[tokio::test]
async fn invalid_station_is_bad_request() {
    let app = TestApp::spawn().await;
    let (status, _) = app
        .get("/rail-api/ntes/train-on-map?train=12055&station=ABCDE")
        .await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn route_only_returns_map_with_coords() {
    let app = TestApp::spawn().await;
    let ntes = app.mocks.get("ntes").unwrap();
    ntes.ntes_web("dummy");
    ntes.route_html_seq("/mntes/TrnMap", vec![ROUTE_HTML.to_string()]);

    let (status, body) = app.get("/rail-api/ntes/train-on-map?train=12055").await;
    assert_eq!(status, 200);
    assert_eq!(body["train_no"], "12055");
    assert_eq!(body["train_name"], "DDN JANSHTBDI");
    assert_eq!(body["source"], "NEW DELHI");
    assert_eq!(body["destination"], "DEHRADOON");
    assert_eq!(body["source_code"], "NDLS");
    assert_eq!(body["dest_code"], "DDN");
    assert_eq!(body["data_source"], "NTES");

    let route = body["route"].as_array().unwrap();
    assert_eq!(route.len(), 3);
    assert_eq!(route[0]["code"], "NDLS");
    assert_eq!(route[0]["name"], "NEW DELHI");
    assert_eq!(route[0]["arrival"], "");
    assert_eq!(route[0]["departure"], "15:20");
    assert_eq!(route[0]["day"], "1");
    assert_eq!(route[0]["distance"], "0");
    assert_eq!(route[0]["days_of_run"], "Daily");
    assert_eq!(route[0]["lat"].as_f64(), Some(28.642464));
    assert_eq!(route[0]["lng"].as_f64(), Some(77.220154));
    assert_eq!(route[1]["code"], "GZB");
    assert_eq!(route[1]["expected_arrival"], "");
    assert_eq!(route[1]["arrival_delay"], "");

    let track = body["track"].as_array().unwrap();
    assert_eq!(track.len(), 6);
    assert_eq!(track[0]["code"], "NDLS");
    assert!(track[0]["lat"].is_number());
    assert!(track[0]["lng"].is_number());

    assert!(body["current_station"].is_null());
    assert!(body["journey_station"].is_null());

    let calls = ntes.calls();
    let trn = calls
        .iter()
        .filter(|(p, _)| p.starts_with("/mntes/TrnMap"))
        .count();
    assert_eq!(trn, 1, "route-only map should POST exactly one TrnMap form");
}

#[tokio::test]
async fn with_station_merges_spot() {
    let app = TestApp::spawn().await;
    let ntes = app.mocks.get("ntes").unwrap();
    ntes.ntes_web("dummy");
    ntes.route_html_seq(
        "/mntes/TrnMap",
        vec![ROUTE_HTML.to_string(), SPOT_HTML.to_string()],
    );

    let (status, body) = app
        .get("/rail-api/ntes/train-on-map?train=12055&station=NDLS")
        .await;
    assert_eq!(status, 200);

    let current = body["current_station"].as_object().unwrap();
    assert_eq!(current["code"], "GZB");
    assert!(current["lat"].is_number());
    assert!(current["lng"].is_number());

    let journey = body["journey_station"].as_object().unwrap();
    assert_eq!(journey["code"], "NDLS");
    assert_eq!(journey["name"], "New Delhi");
    assert_eq!(journey["label"], "Source");
    assert_eq!(journey["expected_arrival"], "17-Aug-2026 15:20");
    assert_eq!(journey["actual_arrival"], "17-Aug-2026 15:20");
    assert_eq!(journey["delay_status"], "On Time");
    assert_eq!(journey["platform"], "9");

    let route = body["route"].as_array().unwrap();
    assert_eq!(route[0]["code"], "NDLS");
    assert_eq!(route[0]["expected_arrival"], "");
    assert_eq!(route[0]["arrival_delay"], "On Time");
    assert_eq!(route[1]["code"], "GZB");
    assert_eq!(route[1]["expected_arrival"], "17-Aug-2026 15:53");
    assert_eq!(route[1]["arrival_delay"], "On Time");
    assert_eq!(route[2]["code"], "DDN");
    assert_eq!(route[2]["expected_departure"], "17-Aug-2026 17:35");
    assert_eq!(route[2]["departure_delay"], "On Time");

    let calls = ntes.calls();
    let trn = calls
        .iter()
        .filter(|(p, _)| p.starts_with("/mntes/TrnMap"))
        .count();
    assert_eq!(trn, 2, "route + spot should POST two TrnMap forms");
}

#[tokio::test]
async fn upstream_failure_is_source_unavailable() {
    let app = TestApp::spawn().await;
    let ntes = app.mocks.get("ntes").unwrap();
    ntes.ntes_web("dummy");

    let (status, body) = app.get("/rail-api/ntes/train-on-map?train=12055").await;
    assert_eq!(status, 502);
    assert!(body["error"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase()
        .contains("source"));
}
