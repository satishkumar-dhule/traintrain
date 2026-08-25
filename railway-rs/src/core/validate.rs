pub fn is_valid_train_1_8(train: &str) -> bool { !train.is_empty() && train.len() <= 8 && train.chars().all(|c| c.is_ascii_digit()) }
pub fn is_valid_train_5(train: &str) -> bool { train.len() == 5 && train.chars().all(|c| c.is_ascii_digit()) && train != "00000" }
pub fn is_valid_train_4_5(train: &str) -> bool { (4..=5).contains(&train.len()) && train.chars().all(|c| c.is_ascii_digit()) }
pub fn is_valid_pnr(pnr: &str) -> bool { pnr.len() == 10 && pnr.chars().all(|c| c.is_ascii_digit()) }
pub fn clamp_query(q: Option<&str>, max: usize) -> String { q.unwrap_or("").chars().take(max).collect() }
pub const MAX_QUERY_LEN: usize = 128;
pub fn clamp_q(q: Option<&str>) -> String { clamp_query(q, MAX_QUERY_LEN) }
#[cfg(test)] mod tests { use super::*; #[test] fn t5(){ assert!(is_valid_train_5("12951")); assert!(!is_valid_train_5("00000")); } }
