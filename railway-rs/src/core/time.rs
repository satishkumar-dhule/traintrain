use chrono::{FixedOffset, NaiveDate};
pub fn ist_offset() -> FixedOffset {
    FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap_or_else(|| FixedOffset::east_opt(0).unwrap())
}
pub fn today_ist() -> String {
    today_ist_date().to_string()
}
pub fn today_ist_date() -> NaiveDate {
    chrono::Utc::now().with_timezone(&ist_offset()).date_naive()
}
pub fn today_ist_ntes() -> String {
    chrono::Utc::now()
        .with_timezone(&ist_offset())
        .format("%d-%b-%Y")
        .to_string()
}
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    for fmt in ["%Y-%m-%d", "%Y%m%d", "%d-%m-%Y", "%d/%m/%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Some(d);
        }
    }
    None
}
pub fn is_valid_date(s: &str) -> bool {
    parse_date(s).is_some()
}
pub fn date_compact(s: &str) -> String {
    parse_date(s)
        .map(|d| d.format("%Y%m%d").to_string())
        .unwrap_or_else(|| s.trim().to_string())
}
pub fn date_iso(s: &str) -> String {
    parse_date(s)
        .map(|d| d.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| s.trim().to_string())
}
pub fn normalize_ntes_date(s: &str) -> String {
    if let Ok(d) = NaiveDate::parse_from_str(s, "%d-%b-%Y") {
        return d.format("%Y-%m-%d").to_string();
    }
    if s.len() == 8 && s.bytes().all(|b| b.is_ascii_digit()) {
        if let Ok(d) = NaiveDate::parse_from_str(s, "%Y%m%d") {
            return d.format("%Y-%m-%d").to_string();
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d.format("%Y-%m-%d").to_string();
    }
    s.to_string()
}
pub fn normalize_pnr_date(s: &str) -> String {
    let s = s.trim();
    for fmt in [
        "%Y-%m-%d",
        "%d-%m-%Y",
        "%d/%m/%Y",
        "%m/%d/%Y",
        "%b %d, %Y %I:%M:%S %p",
        "%b %d, %Y %I:%M %p",
        "%b %d, %Y",
    ] {
        if let Ok(d) = chrono::NaiveDateTime::parse_from_str(s, fmt) {
            return d.date().to_string();
        }
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return d.to_string();
        }
    }
    s.to_string()
}
