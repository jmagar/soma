//! Minimal shape for starting a thread and sending one text turn.
//!
//! This can consume model credits, so run it deliberately:
//! `cargo run -p codex-app-server-client --example session_turn -- "say hi"`

use codex_app_server_client::protocol::{ThreadStartParams, TurnStartParams};
use codex_app_server_client::{
    CodexSession, DenyAllApprovalHandler, EventCollector, SessionOptions,
};

#[tokio::main]
async fn main() -> codex_app_server_client::Result<()> {
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Say hello in one sentence.".to_owned());
    let mut session = CodexSession::spawn(SessionOptions::new(
        "codex_app_server_client_turn_example",
        env!("CARGO_PKG_VERSION"),
    ))
    .await?;

    let thread = session
        .start_thread(ThreadStartParams::new().model("gpt-5"))
        .await?;
    let turn = session
        .send_turn(TurnStartParams::text(&thread.thread.id, prompt))
        .await?;

    let mut collector = EventCollector::for_turn(&thread.thread.id, &turn.turn.id);
    session
        .collect_until_complete(&mut collector, &DenyAllApprovalHandler::default())
        .await?;

    println!("{}", collector.agent_message());
    Ok(())
}
