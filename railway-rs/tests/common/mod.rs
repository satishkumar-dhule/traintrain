//! Shared test harness: spawn the real app against mock upstreams and drive
//! it over HTTP. Every network-dependent slice test must point its source
//! bases at a `MockServer` so tests are hermetic and fast.
#![allow(dead_code)]

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};

use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, Response, StatusCode};
use axum::Router;
use serde_json::{json, Value};

use railway_rs::config::Config;
use railway_rs::state::AppState;
use railway_rs::web;

/// A canned upstream response.
#[derive(Clone)]
pub struct RouteSpec {
    pub status: StatusCode,
    pub body: Value,
    pub content_type: String,
}

struct MockInner {
    addr: OnceLock<SocketAddr>,
    routes: Mutex<HashMap<String, RouteSpec>>,
}

/// A local mock of one of the upstream providers (Railyatri / etrain / NTES).
/// Routes are registered by path prefix and can be updated while the app runs.
#[derive(Clone)]
pub struct MockServer {
    inner: Arc<MockInner>,
}

impl MockServer {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(MockInner {
                addr: OnceLock::new(),
                routes: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Start the HTTP listener and return its address.
    pub async fn spawn(&self) -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        self.inner.addr.set(addr).ok();
        let state = self.clone();
        tokio::spawn(async move {
            let router = Router::new()
                .route("/*path", axum::routing::any(mock_handler))
                .with_state(state);
            axum::serve(listener, router).await.unwrap();
        });
        addr
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr())
    }

    pub fn addr(&self) -> SocketAddr {
        *self
            .inner
            .addr
            .get()
            .expect("MockServer must be spawned() first")
    }

    pub fn route(&self, path_prefix: &str, spec: RouteSpec) {
        self.inner
            .routes
            .lock()
            .unwrap()
            .insert(path_prefix.to_string(), spec);
    }

    pub fn route_json(&self, path_prefix: &str, body: Value) {
        self.route(
            path_prefix,
            RouteSpec {
                status: StatusCode::OK,
                body,
                content_type: "application/json".to_string(),
            },
        );
    }

    pub fn route_html(&self, path_prefix: &str, html: impl Into<String>) {
        self.route(
            path_prefix,
            RouteSpec {
                status: StatusCode::OK,
                body: Value::String(html.into()),
                content_type: "text/html".to_string(),
            },
        );
    }

    pub fn route_error(&self, path_prefix: &str, status: StatusCode) {
        self.route(
            path_prefix,
            RouteSpec {
                status,
                body: json!({"error": "mock failure"}),
                content_type: "application/json".to_string(),
            },
        );
    }

    fn lookup(&self, path: &str) -> Option<RouteSpec> {
        let routes = self.inner.routes.lock().unwrap();
        let mut best: Option<(&String, &RouteSpec)> = None;
        for (prefix, spec) in routes.iter() {
            if path.starts_with(prefix.as_str()) {
                match best {
                    Some((bp, _)) if bp.len() > prefix.len() => {}
                    _ => best = Some((prefix, spec)),
                }
            }
        }
        best.map(|(_, s)| s.clone())
    }
}

impl Default for MockServer {
    fn default() -> Self {
        Self::new()
    }
}

async fn mock_handler(State(m): State<MockServer>, req: Request<Body>) -> Response<Body> {
    let path = req.uri().path().to_string();
    match m.lookup(&path) {
        Some(spec) => {
            let body = if spec.content_type.starts_with("application/json") {
                Body::from(serde_json::to_vec(&spec.body).unwrap())
            } else {
                Body::from(spec.body.as_str().unwrap_or_default().to_string())
            };
            let mut builder = Response::builder().status(spec.status);
            if let Some(headers) = builder.headers_mut() {
                headers.insert("content-type", spec.content_type.parse().unwrap());
            }
            builder.body(body).unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("mock: no route for this path"))
            .unwrap(),
    }
}

/// The app under test bound to an ephemeral port, with four live mock
/// upstreams wired into the config (railyatri / etrain / ntes / ir).
pub struct TestApp {
    pub addr: SocketAddr,
    pub state: AppState,
    pub mocks: HashMap<String, MockServer>,
}

impl TestApp {
    /// Spawn the real web app plus the three mock upstreams.
    pub async fn spawn() -> Self {
        Self::spawn_with_config(Config::default()).await
    }

    /// Spawn with a caller-provided config (e.g. custom data dir).
    pub async fn spawn_with_config(mut config: Config) -> Self {
        let mut mocks = HashMap::new();
        for name in ["railyatri", "etrain", "ntes", "ir"] {
            let m = MockServer::new();
            m.spawn().await;
            mocks.insert(name.to_string(), m);
        }

        config.railyatri_base = mocks["railyatri"].base_url();
        config.etrain_base = mocks["etrain"].base_url();
        config.ntes_base = mocks["ntes"].base_url();
        config.ir_base = mocks["ir"].base_url();

        let state = AppState::from_config(config).expect("state builds");
        let app = web::router(state.clone(), state.config.static_dir.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self { addr, state, mocks }
    }

    pub fn base_url(&self) -> String {
        format!("http://{}", self.addr)
    }

    pub fn mock(&self, name: &str) -> &MockServer {
        &self.mocks[name]
    }

    pub async fn get(&self, path: &str) -> (StatusCode, Value) {
        let resp = reqwest::get(format!("{}{}", self.base_url(), path))
            .await
            .expect("request to app");
        let status = resp.status();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        (status, body)
    }

    pub async fn get_raw(&self, path: &str) -> (StatusCode, String) {
        let resp = reqwest::get(format!("{}{}", self.base_url(), path))
            .await
            .expect("request to app");
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        (status, text)
    }
}
