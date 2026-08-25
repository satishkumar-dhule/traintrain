use reqwest::header::SET_COOKIE;
use std::sync::{Arc, Mutex};
#[derive(Debug, Clone, Default)]
pub struct CookieJar(pub Arc<Mutex<Vec<(String, String)>>>);
impl CookieJar {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(Vec::new())))
    }
    pub fn ingest_response(&self, res: &reqwest::Response) {
        let mut jar = self.0.lock().unwrap();
        for raw in res
            .headers()
            .get_all(SET_COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
        {
            if let Some(pair) = raw.split(';').next() {
                if let Some((n, v)) = pair.split_once('=') {
                    let n = n.trim();
                    if n.is_empty() {
                        continue;
                    }
                    merge_cookie(&mut jar, n.to_string(), v.trim().to_string());
                }
            }
        }
    }
    pub fn merge(&self, name: String, value: String) {
        if name.is_empty() {
            return;
        }
        let mut jar = self.0.lock().unwrap();
        merge_cookie(&mut jar, name, value);
    }
    pub fn header_value(&self) -> Option<String> {
        let jar = self.0.lock().unwrap();
        if jar.is_empty() {
            None
        } else {
            Some(
                jar.iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("; "),
            )
        }
    }
    pub fn cookie_str(&self) -> String {
        self.header_value().unwrap_or_default()
    }
    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.0.lock().unwrap().clone()
    }
    pub fn restore(&self, c: Vec<(String, String)>) {
        *self.0.lock().unwrap() = c;
    }
}
fn merge_cookie(jar: &mut Vec<(String, String)>, name: String, value: String) {
    if name.is_empty() {
        return;
    }
    if let Some(e) = jar.iter_mut().find(|(k, _)| *k == name) {
        e.1 = value
    } else {
        jar.push((name, value))
    }
}
pub fn cookie_str(c: &[(String, String)]) -> String {
    c.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("; ")
}
pub fn capture_cookies(res: &reqwest::Response) -> Vec<(String, String)> {
    res.headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .filter_map(|s| s.split(';').next())
        .filter_map(|pair| {
            let (n, v) = pair.split_once('=')?;
            let n = n.trim();
            if n.is_empty() {
                None
            } else {
                Some((n.to_string(), v.trim().to_string()))
            }
        })
        .collect()
}
