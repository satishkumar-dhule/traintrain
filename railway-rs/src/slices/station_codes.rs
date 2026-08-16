//! Shared station-code validation, used by every slice that takes station
//! codes (`trains_between`, `availability`). Kept here so the rules live in
//! exactly one place (DRY).

use crate::core::error::AppError;
use crate::state::AppState;

/// Trim + uppercase a station code query value (empty when absent).
pub fn normalize_code(code: Option<&str>) -> String {
    code.unwrap_or_default().trim().to_uppercase()
}

pub fn is_valid_code(code: &str) -> bool {
    code.len() == 4 && code.chars().all(|c| c.is_ascii_alphanumeric())
}

/// A code is known when it matches a station in the local dataset, or appears
/// as a token in an official NTES train name (e.g. `MMCT` in
/// `"MMCT NDLS RAJDHANI"`) - such tokens denote real stations.
pub fn code_known(state: &AppState, code: &str) -> bool {
    state
        .datasets
        .stations
        .iter()
        .any(|s| s.code.eq_ignore_ascii_case(code))
        || state.datasets.trains.iter().any(|t| {
            t.name
                .split_whitespace()
                .any(|tok| tok.trim_matches('-').eq_ignore_ascii_case(code))
        })
}

/// Validate a required station code, returning the normalized uppercase form.
pub fn require_station(
    state: &AppState,
    raw: Option<&str>,
    param: &str,
) -> Result<String, AppError> {
    let code = normalize_code(raw);
    if code.is_empty() {
        return Err(AppError::bad_request(format!(
            "Missing required query parameter: {param}"
        )));
    }
    if !is_valid_code(&code) {
        return Err(AppError::bad_request(format!(
            "Invalid station code: {code}"
        )));
    }
    if !code_known(state, &code) {
        return Err(AppError::bad_request(format!("Station {code} not found.")));
    }
    Ok(code)
}
