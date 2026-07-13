use std::sync::{Arc, Mutex};

use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
use serde_json::{json, Value};
use tempfile::tempdir;
use tokio::net::TcpListener;

#[tokio::test]
async fn remote_cli_dynamic_action_posts_to_server_api() -> anyhow::Result<()> {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let (base_url, handle) = mock_api(observed.clone()).await?;
    let home = tempdir()?;

    let output = tokio::process::Command::new(env!("CARGO_BIN_EXE_soma"))
        .args(["weather-current", "--city", "Paris", "--units", "metric"])
        .env("HOME", home.path())
        .env("SOMA_HOME", home.path())
        .env("SOMA_RUNTIME_MODE", "remote")
        .env("SOMA_API_URL", base_url)
        .env("SOMA_API_KEY", "remote-secret")
        .env("RUST_LOG", "warn")
        .env_remove("SOMA_PROVIDER_DIR")
        .output()
        .await?;

    handle.abort();

    assert!(
        output.status.success(),
        "status: {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout: Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(stdout["ok"], true);
    assert_eq!(stdout["city"], "Paris");

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
