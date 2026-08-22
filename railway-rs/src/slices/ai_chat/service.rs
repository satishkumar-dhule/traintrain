//! AI assistant service logic (Phase B fills the chat relay; status is live).

use crate::state::AppState;

use super::AiStatus;

/// Configuration-derived status. No upstream call: the SPA gates on this
/// instantly and honest errors arrive per-request if the gateway is down.
pub fn status(state: &AppState) -> AiStatus {
    AiStatus {
        enabled: state.config.ai_enabled,
        model: state.config.ai_model.clone(),
        keyed: state.config.ai_api_key.is_some(),
        base: state.config.ai_base.clone(),
    }
}
