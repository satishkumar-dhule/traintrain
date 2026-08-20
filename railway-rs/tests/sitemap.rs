mod common;

use common::TestApp;

#[tokio::test]
async fn sitemap_lists_canonical_pages() {
    let app = TestApp::spawn().await;
    let (status, body) = app.get_raw("/sitemap.xml").await;
    assert_eq!(status, 200);
    assert!(body.starts_with("<?xml version=\"1.0\""));
    assert!(body.contains("<urlset"));
    let addr = app.addr;
    for path in [
        "/",
        "/#/train",
        "/#/station",
        "/#/station/heritage",
        "/#/station/parcel",
        "/#/plan",
        "/#/system",
        "/#/system/observability",
        "/#/system/settings",
        "/#/system/debug",
    ] {
        assert!(
            body.contains(&format!("<loc>http://{addr}{path}</loc>")),
            "missing {path} in sitemap",
        );
    }
}

#[tokio::test]
async fn sitemap_honors_forwarded_proto_and_host() {
    let app = TestApp::spawn().await;
    let resp = reqwest::Client::new()
        .get(format!("http://{}/sitemap.xml", app.addr))
        .header("host", "rail.example.com")
        .header("x-forwarded-proto", "https")
        .send()
        .await
        .expect("request to app");
    let body = resp.text().await.unwrap_or_default();
    assert!(
        body.contains("<loc>https://rail.example.com/</loc>"),
        "expected https://rail.example.com/ in body: {body}",
    );
    assert!(
        body.contains("<loc>https://rail.example.com/#/plan</loc>"),
        "expected #/plan entry in body: {body}",
    );
}
