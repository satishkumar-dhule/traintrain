//! stdio MCP server: exposes railway-rs's local rail tools (trains-between,
//! live status, average delay, BM25 corpus search) to any MCP client —
//! Claude Desktop, other agents, CLIs.
//!
//! Wire format: newline-delimited JSON-RPC 2.0 on stdin/stdout. Run it with
//! any process launcher; e.g. in an MCP client config:
//!
//! ```json
//! { "mcpServers": { "railway-rs": { "command": "/opt/railway-rs/railway-mcp" } } }
//! ```

use railway_rs::config::Config;
use railway_rs::mcp;
use railway_rs::state::AppState;

#[tokio::main]
async fn main() {
    let state = AppState::from_config(Config::from_env()).unwrap_or_else(|e| {
        eprintln!("railway-mcp: failed to build app state: {e}");
        std::process::exit(1);
    });

    use tokio::io::{AsyncBufReadExt, BufReader};
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let msg: serde_json::Value = match serde_json::from_str(trimmed) {
            Ok(v) => v,
            // Unparseable lines are ignored: some clients emit keepalives.
            Err(_) => continue,
        };
        if let Some(resp) = mcp::handle(&state, &msg).await {
            println!("{}", serde_json::to_string(&resp).unwrap_or_default());
        }
    }
}
