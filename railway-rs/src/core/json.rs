use serde_json::Value;
pub trait ValueExt {
    fn str_field(&self, key: &str) -> String;
    fn opt_str(&self, key: &str) -> Option<String>;
    fn str_one_of(&self, keys: &[&str]) -> String;
    fn i64_one_of(&self, keys: &[&str]) -> Option<i64>;
}
impl ValueExt for Value {
    fn str_field(&self, key: &str) -> String {
        self.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    }
    fn opt_str(&self, key: &str) -> Option<String> {
        self.get(key)
            .and_then(Value::as_str)
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
    }
    fn str_one_of(&self, keys: &[&str]) -> String {
        keys.iter()
            .find_map(|k| self.get(*k).and_then(Value::as_str))
            .unwrap_or_default()
            .to_string()
    }
    fn i64_one_of(&self, keys: &[&str]) -> Option<i64> {
        keys.iter().find_map(|k| match self.get(*k) {
            Some(Value::Number(n)) => n.as_i64(),
            Some(Value::String(s)) => s.parse().ok(),
            _ => None,
        })
    }
}
pub fn str_field(v: &Value, key: &str) -> String {
    v.str_field(key)
}
pub fn day_bool(entry: &Value, day: &str) -> bool {
    entry
        .get(format!("runOn{day}"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || entry
            .get(format!("runsOn{day}"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
}
