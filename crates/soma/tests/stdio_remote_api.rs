use std::{
    process::Stdio,
    sync::{Arc, Mutex},
};

use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use rmcp::{
    model::CallToolRequestParams,
    transport::{ConfigureCommandExt, TokioChildProcess},
    ServiceExt,
};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::{net::TcpListener, process::Command};

#[tokio::test]
async fn remote_stdio_mcp_provider_action_posts_to_server_api() -> anyhow::Result<()> {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let (base_url, handle) = mock_api(observed.clone()).await?;
    let home = tempdir()?;
    let bad_provider_dir = home.path().join("providers");
    std::fs::create_dir(&bad_provider_dir)?;
    std::fs::write(bad_provider_dir.join("broken.json"), "{ not json")?;

    let (transport, _stderr) =
        TokioChildProcess::builder(Command::new(env!("CARGO_BIN_EXE_soma")).configure(|cmd| {
            cmd.arg("mcp")
                .env("HOME", home.path())
                .env("SOMA_HOME", home.path())
                .env("SOMA_RUNTIME_MODE", "remote")
                .env("SOMA_API_URL", &base_url)
                .env("SOMA_API_KEY", "remote-secret")
                .env("RUST_LOG", "warn")
                .env("SOMA_PROVIDER_DIR", &bad_provider_dir);
        }))
        .stderr(Stdio::null())
        .spawn()?;
    let service = ().serve(transport).await?;

    let result = service
        .call_tool(
            CallToolRequestParams::new("soma").with_arguments(
                json!({
                    "action": "weather-current",
                    "city": "Paris",
                    "units": "metric"
                })
                .as_object()
                .unwrap()
                .clone(),
            ),
        )
        .await?;
    service.cancel().await?;
    handle.abort();

    assert_eq!(result.structured_content.unwrap()["ok"], true);
    let observed = observed.lock().expect("observed requests should lock");
    assert_eq!(observed.len(), 1);
    assert_eq!(observed[0].path, "/v1/weather-current");
    assert_eq!(observed[0].bearer, "Bearer remote-secret");
    assert_eq!(observed[0].body["city"], "Paris");
    assert_eq!(observed[0].body["units"], "metric");
    Ok(())
}

#[derive(Debug, Clone)]
struct ObservedRequest {
    path: String,
    bearer: String,
    body: Value,
}

type ObservedRequests = Arc<Mutex<Vec<ObservedRequest>>>;

async fn mock_api(
    observed: ObservedRequests,
) -> anyhow::Result<(String, tokio::task::JoinHandle<std::io::Result<()>>)> {
    let app = Router::new()
        .route("/v1/weather-current", post(mock_weather))
        .with_state(observed);
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move { axum::serve(listener, app.into_make_service()).await });
    Ok((format!("http://{addr}/"), handle))
}

async fn mock_weather(
    State(observed): State<ObservedRequests>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Json<Value> {
    let bearer = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    observed
        .lock()
        .expect("observed requests should lock")
        .push(ObservedRequest {
            path: "/v1/weather-current".to_owned(),
            bearer,
            body: body.clone(),
        });
    Json(json!({
        "ok": true,
        "city": body["city"],
        "units": body["units"]
    }))
}
