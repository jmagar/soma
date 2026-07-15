use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    AllowAllApprovalHandler, CodexAppServerClient, CodexSession, CompatibilityReport,
    DenyAllApprovalHandler, Error, Event, PendingServerRequest, ReadOnlyApprovalHandler, Result,
    SessionOptions, TextTurnResult,
};

pub type RestResult<T> = std::result::Result<T, RestError>;
pub type RestFuture<T> = Pin<Box<dyn Future<Output = RestResult<T>> + Send + 'static>>;

/// Errors surfaced by the optional REST adapter.
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    #[error("{0}")]
    NotFound(String),

    #[error(transparent)]
    Client(#[from] Error),
}

impl From<serde_json::Error> for RestError {
    fn from(error: serde_json::Error) -> Self {
        Self::Client(error.into())
    }
}

/// Backend used by the REST router.
///
/// The default [`CodexRestBackend`] talks to real `codex app-server`
/// processes. Tests and host applications can inject their own backend with
/// [`router_with_backend`] to control process lifecycle, pooling, or policy.
pub trait RestBackend: Send + Sync + 'static {
    fn compatibility_report(&self) -> CompatibilityReport;
    fn run_text_turn(&self, request: RestTextTurnRequest) -> RestFuture<RestTextTurnResponse>;
    fn create_session(
        &self,
        request: RestSessionCreateRequest,
    ) -> RestFuture<RestSessionCreateResponse>;
    fn list_sessions(&self) -> RestFuture<Vec<RestSessionSummary>>;
    fn delete_session(&self, session_id: String) -> RestFuture<RestStatusResponse>;
    fn call_method(&self, request: RestCallRequest) -> RestFuture<RestCallResponse>;
    fn poll_event(
        &self,
        session_id: String,
        timeout_ms: Option<u64>,
    ) -> RestFuture<RestEventResponse>;
    fn reply_request_result(
        &self,
        session_id: String,
        request_key: String,
        body: RestRequestReplyResultRequest,
    ) -> RestFuture<RestRequestReplyResponse>;
    fn reply_request_error(
        &self,
        session_id: String,
        request_key: String,
        body: RestErrorReplyRequest,
    ) -> RestFuture<RestRequestReplyResponse>;
}

/// Production REST backend.
///
/// One-shot calls create a short-lived Codex session. Stateful bridge calls use
/// sessions created by `POST /v1/sessions`.
#[derive(Clone, Default)]
pub struct CodexRestBackend {
    sessions: Arc<CodexRestSessions>,
}

#[derive(Default)]
struct CodexRestSessions {
    next_session_id: AtomicU64,
    sessions: Mutex<HashMap<String, Arc<CodexRestSession>>>,
}

struct CodexRestSession {
    client: CodexAppServerClient,
    session: Mutex<CodexSession>,
    pending_requests: Mutex<HashMap<String, PendingServerRequest>>,
    next_request_key: AtomicU64,
}

impl CodexRestBackend {
    async fn session(&self, session_id: &str) -> RestResult<Arc<CodexRestSession>> {
        self.sessions
            .sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| RestError::NotFound(format!("session `{session_id}` was not found")))
    }
}

impl RestBackend for CodexRestBackend {
    fn compatibility_report(&self) -> CompatibilityReport {
        CompatibilityReport::current()
    }

    fn run_text_turn(&self, request: RestTextTurnRequest) -> RestFuture<RestTextTurnResponse> {
        Box::pin(async move {
            let session_options = request.session_options();
            let approval_policy = request.approval_policy.unwrap_or_default();
            let model = request.model.clone();
            let prompt = request.prompt.clone();
            let mut session = CodexSession::spawn(session_options).await?;

            let result = match approval_policy {
                RestApprovalPolicy::DenyAll => {
                    run_text_turn_with_handler(
                        &mut session,
                        model,
                        prompt,
                        &DenyAllApprovalHandler::default(),
                    )
                    .await?
                }
                RestApprovalPolicy::ReadOnly => {
                    run_text_turn_with_handler(
                        &mut session,
                        model,
                        prompt,
                        &ReadOnlyApprovalHandler,
                    )
                    .await?
                }
                RestApprovalPolicy::AllowAll => {
                    run_text_turn_with_handler(
                        &mut session,
                        model,
                        prompt,
                        &AllowAllApprovalHandler,
                    )
                    .await?
                }
            };
            Ok(RestTextTurnResponse::from(result))
        })
    }

    fn create_session(
        &self,
        request: RestSessionCreateRequest,
    ) -> RestFuture<RestSessionCreateResponse> {
        let backend = self.clone();
        Box::pin(async move {
            let session = CodexSession::spawn(session_options_from(
                request.client,
                "codex_app_server_rest_session",
            ))
            .await?;
            let initialize_response = serde_json::to_value(session.initialize_response())?;
            let client = session.client().clone();
            let session_id = format!(
                "session-{}",
                backend
                    .sessions
                    .next_session_id
                    .fetch_add(1, Ordering::Relaxed)
                    + 1
            );
            let rest_session = Arc::new(CodexRestSession {
                client,
                session: Mutex::new(session),
                pending_requests: Mutex::new(HashMap::new()),
                next_request_key: AtomicU64::new(0),
            });
            backend
                .sessions
                .sessions
                .lock()
                .await
                .insert(session_id.clone(), rest_session);
            Ok(RestSessionCreateResponse {
                session_id,
                initialize_response,
            })
        })
    }

    fn list_sessions(&self) -> RestFuture<Vec<RestSessionSummary>> {
        let backend = self.clone();
        Box::pin(async move {
            let mut sessions = backend
                .sessions
                .sessions
                .lock()
                .await
                .keys()
                .map(|session_id| RestSessionSummary {
                    session_id: session_id.clone(),
                })
                .collect::<Vec<_>>();
            sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
            Ok(sessions)
        })
    }

    fn delete_session(&self, session_id: String) -> RestFuture<RestStatusResponse> {
        let backend = self.clone();
        Box::pin(async move {
            let removed = backend
                .sessions
                .sessions
                .lock()
                .await
                .remove(&session_id)
                .is_some();
            if removed {
                Ok(RestStatusResponse {
                    status: "deleted".to_owned(),
                })
            } else {
                Err(RestError::NotFound(format!(
                    "session `{session_id}` was not found"
                )))
            }
        })
    }

    fn call_method(&self, request: RestCallRequest) -> RestFuture<RestCallResponse> {
        let backend = self.clone();
        Box::pin(async move {
            let method = request.method.clone();
            let result = if let Some(session_id) = request.session_id.as_deref() {
                let session = backend.session(session_id).await?;
                session
                    .client
                    .call_raw_method(method.clone(), request.params)
                    .await?
            } else {
                let session = CodexSession::spawn(session_options_from(
                    request.client,
                    "codex_app_server_rest_call",
                ))
                .await?;
                session
                    .client()
                    .call_raw_method(method.clone(), request.params)
                    .await?
            };
            Ok(RestCallResponse { method, result })
        })
    }

    fn poll_event(
        &self,
        session_id: String,
        timeout_ms: Option<u64>,
    ) -> RestFuture<RestEventResponse> {
        let backend = self.clone();
        Box::pin(async move {
            let session = backend.session(&session_id).await?;
            let timeout = Duration::from_millis(timeout_ms.unwrap_or(30_000));
            let event = tokio::time::timeout(timeout, async {
                let mut session_guard = session.session.lock().await;
                session_guard.next_event().await
            })
            .await;

            match event {
                Ok(Some(Event::Notification(notification))) => Ok(RestEventResponse::notification(
                    serde_json::to_value(notification)?,
                )),
                Ok(Some(Event::Request(request))) => {
                    let request_key = format!(
                        "request-{}",
                        session.next_request_key.fetch_add(1, Ordering::Relaxed) + 1
                    );
                    let request_id = serde_json::to_value(request.id())?;
                    let method = request.method_name().to_owned();
                    let request_value = serde_json::to_value(&request.request)?;
                    session
                        .pending_requests
                        .lock()
                        .await
                        .insert(request_key.clone(), request);
                    Ok(RestEventResponse::request(
                        request_key,
                        request_id,
                        method,
                        request_value,
                    ))
                }
                Ok(Some(Event::Closed)) | Ok(None) => Ok(RestEventResponse::closed()),
                Err(_elapsed) => Ok(RestEventResponse::timeout()),
            }
        })
    }

    fn reply_request_result(
        &self,
        session_id: String,
        request_key: String,
        body: RestRequestReplyResultRequest,
    ) -> RestFuture<RestRequestReplyResponse> {
        let backend = self.clone();
        Box::pin(async move {
            let session = backend.session(&session_id).await?;
            let pending = session
                .pending_requests
                .lock()
                .await
                .remove(&request_key)
                .ok_or_else(|| {
                    RestError::NotFound(format!("request `{request_key}` was not found"))
                })?;
            pending.respond(body.result)?;
            Ok(RestRequestReplyResponse {
                status: "ok".to_owned(),
            })
        })
    }

    fn reply_request_error(
        &self,
        session_id: String,
        request_key: String,
        body: RestErrorReplyRequest,
    ) -> RestFuture<RestRequestReplyResponse> {
        let backend = self.clone();
        Box::pin(async move {
            let session = backend.session(&session_id).await?;
            let pending = session
                .pending_requests
                .lock()
                .await
                .remove(&request_key)
                .ok_or_else(|| {
                    RestError::NotFound(format!("request `{request_key}` was not found"))
                })?;
            pending.respond_error(body.code, body.message, body.data);
            Ok(RestRequestReplyResponse {
                status: "ok".to_owned(),
            })
        })
    }
}

#[derive(Clone)]
struct RestState {
    backend: Arc<dyn RestBackend>,
}

/// Builds a REST router backed by real `codex app-server` processes.
///
/// Routes:
/// - `GET /health`
/// - `GET /v1/health`
/// - `GET /v1/compatibility`
/// - `POST /v1/text-turn`
/// - `POST /v1/call/{method}`
/// - `GET|POST /v1/sessions`
/// - `DELETE /v1/sessions/{sessionId}`
/// - `POST /v1/sessions/{sessionId}/call/{method}`
/// - `GET /v1/sessions/{sessionId}/events`
/// - `POST /v1/sessions/{sessionId}/requests/{requestKey}/result`
/// - `POST /v1/sessions/{sessionId}/requests/{requestKey}/error`
pub fn router() -> Router {
    router_with_backend(CodexRestBackend::default())
}

/// Builds a REST router with a caller-provided backend.
pub fn router_with_backend<B>(backend: B) -> Router
where
    B: RestBackend,
{
    router_with_backend_arc(Arc::new(backend))
}

/// Builds a REST router from a shared backend trait object.
pub fn router_with_backend_arc(backend: Arc<dyn RestBackend>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/health", get(health))
        .route("/v1/compatibility", get(compatibility))
        .route("/v1/text-turn", post(text_turn))
        .route("/v1/call/{*method}", post(call_method))
        .route("/v1/sessions", get(list_sessions).post(create_session))
        .route("/v1/sessions/{session_id}", delete(delete_session))
        .route(
            "/v1/sessions/{session_id}/call/{*method}",
            post(call_session_method),
        )
        .route("/v1/sessions/{session_id}/events", get(poll_event))
        .route(
            "/v1/sessions/{session_id}/requests/{request_key}/result",
            post(reply_request_result),
        )
        .route(
            "/v1/sessions/{session_id}/requests/{request_key}/error",
            post(reply_request_error),
        )
        .with_state(RestState { backend })
}

/// Health response returned by `GET /health` and `GET /v1/health`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RestHealthResponse {
    pub status: String,
}

/// REST approval policy preset used while collecting turn events.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RestApprovalPolicy {
    #[default]
    DenyAll,
    ReadOnly,
    AllowAll,
}

/// Optional client/session overrides for REST requests that spawn Codex.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestClientOptions {
    pub name: Option<String>,
    pub version: Option<String>,
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_args: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub config: HashMap<String, String>,
    pub call_timeout_ms: Option<u64>,
}

impl RestClientOptions {
    fn into_session_options(self, default_name: &str) -> SessionOptions {
        let mut options = SessionOptions::new(
            self.name.unwrap_or_else(|| default_name.to_owned()),
            self.version
                .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned()),
        );
        if let Some(command) = self.command {
            options = options.with_command(command);
        }
        for arg in self.extra_args {
            options = options.with_extra_arg(arg);
        }
        for (key, value) in self.config {
            options = options.with_config(key, value);
        }
        if let Some(timeout_ms) = self.call_timeout_ms {
            options = options.with_call_timeout(Duration::from_millis(timeout_ms));
        }
        options
    }
}

/// Request body for `POST /v1/text-turn`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestTextTurnRequest {
    pub prompt: String,
    pub model: Option<String>,
    pub approval_policy: Option<RestApprovalPolicy>,
    pub client: Option<RestClientOptions>,
}

impl RestTextTurnRequest {
    pub fn session_options(&self) -> SessionOptions {
        session_options_from(self.client.clone(), "codex_app_server_rest")
    }
}

/// Response body for `POST /v1/text-turn`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestTextTurnResponse {
    pub thread_id: String,
    pub turn_id: String,
    pub agent_message: String,
    pub latest_diff: Option<String>,
    pub errors: Vec<String>,
}

impl From<TextTurnResult> for RestTextTurnResponse {
    fn from(result: TextTurnResult) -> Self {
        let agent_message = result.agent_message().to_owned();
        let latest_diff = result.latest_diff().map(str::to_owned);
        let errors = result
            .errors()
            .iter()
            .map(|error| error.message.clone())
            .collect();
        Self {
            thread_id: result.thread.thread.id,
            turn_id: result.turn.turn.id,
            agent_message,
            latest_diff,
            errors,
        }
    }
}

/// Request body for raw method bridge calls.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestCallBody {
    #[serde(default = "empty_object")]
    pub params: Value,
    pub client: Option<RestClientOptions>,
}

/// Backend-facing raw method call request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestCallRequest {
    pub session_id: Option<String>,
    pub method: String,
    pub params: Value,
    pub client: Option<RestClientOptions>,
}

/// Response body for raw method bridge calls.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestCallResponse {
    pub method: String,
    pub result: Value,
}

/// Request body for `POST /v1/sessions`.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestSessionCreateRequest {
    pub client: Option<RestClientOptions>,
}

/// Response body for `POST /v1/sessions`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestSessionCreateResponse {
    pub session_id: String,
    pub initialize_response: Value,
}

/// One session entry in `GET /v1/sessions`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestSessionSummary {
    pub session_id: String,
}

/// Response body for `GET /v1/sessions`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RestListSessionsResponse {
    pub sessions: Vec<RestSessionSummary>,
}

/// Simple status response used by mutating bridge endpoints.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RestStatusResponse {
    pub status: String,
}

/// Event returned by `GET /v1/sessions/{sessionId}/events`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestEventResponse {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request: Option<Value>,
}

impl RestEventResponse {
    pub fn notification(notification: Value) -> Self {
        Self {
            event: "notification".to_owned(),
            notification: Some(notification),
            request_key: None,
            request_id: None,
            method: None,
            request: None,
        }
    }

    pub fn request(
        request_key: impl Into<String>,
        request_id: Value,
        method: impl Into<String>,
        request: Value,
    ) -> Self {
        Self {
            event: "request".to_owned(),
            notification: None,
            request_key: Some(request_key.into()),
            request_id: Some(request_id),
            method: Some(method.into()),
            request: Some(request),
        }
    }

    pub fn closed() -> Self {
        Self {
            event: "closed".to_owned(),
            notification: None,
            request_key: None,
            request_id: None,
            method: None,
            request: None,
        }
    }

    pub fn timeout() -> Self {
        Self {
            event: "timeout".to_owned(),
            notification: None,
            request_key: None,
            request_id: None,
            method: None,
            request: None,
        }
    }
}

/// Request body for replying successfully to a server-originated request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestRequestReplyResultRequest {
    pub result: Value,
}

/// Request body for replying with an error to a server-originated request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestErrorReplyRequest {
    pub code: i64,
    pub message: String,
    pub data: Option<Value>,
}

/// Response body for request-reply endpoints.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RestRequestReplyResponse {
    pub status: String,
}

/// Structured REST error payload.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RestErrorResponse {
    pub error: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EventQuery {
    timeout_ms: Option<u64>,
}

async fn health() -> impl IntoResponse {
    Json(RestHealthResponse {
        status: "ok".to_owned(),
    })
}

async fn compatibility(State(state): State<RestState>) -> impl IntoResponse {
    Json(state.backend.compatibility_report())
}

async fn text_turn(
    State(state): State<RestState>,
    body: std::result::Result<Json<RestTextTurnRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match body {
        Ok(body) => body,
        Err(error) => return invalid_json(error),
    };
    if request.prompt.trim().is_empty() {
        return invalid_request("prompt must not be empty");
    }

    match state.backend.run_text_turn(request).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn call_method(
    State(state): State<RestState>,
    Path(method): Path<String>,
    body: std::result::Result<Json<RestCallBody>, JsonRejection>,
) -> Response {
    let method = match normalize_method(method) {
        Some(method) => method,
        None => return invalid_request("method path must not be empty"),
    };
    let Json(body) = match body {
        Ok(body) => body,
        Err(error) => return invalid_json(error),
    };
    let request = RestCallRequest {
        session_id: None,
        method,
        params: body.params,
        client: body.client,
    };
    match state.backend.call_method(request).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn create_session(
    State(state): State<RestState>,
    body: std::result::Result<Json<RestSessionCreateRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match body {
        Ok(body) => body,
        Err(error) => return invalid_json(error),
    };
    match state.backend.create_session(request).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn list_sessions(State(state): State<RestState>) -> Response {
    match state.backend.list_sessions().await {
        Ok(sessions) => Json(RestListSessionsResponse { sessions }).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn delete_session(
    State(state): State<RestState>,
    Path(session_id): Path<String>,
) -> Response {
    match state.backend.delete_session(session_id).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn call_session_method(
    State(state): State<RestState>,
    Path((session_id, method)): Path<(String, String)>,
    body: std::result::Result<Json<RestCallBody>, JsonRejection>,
) -> Response {
    let method = match normalize_method(method) {
        Some(method) => method,
        None => return invalid_request("method path must not be empty"),
    };
    let Json(body) = match body {
        Ok(body) => body,
        Err(error) => return invalid_json(error),
    };
    let request = RestCallRequest {
        session_id: Some(session_id),
        method,
        params: body.params,
        client: body.client,
    };
    match state.backend.call_method(request).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn poll_event(
    State(state): State<RestState>,
    Path(session_id): Path<String>,
    Query(query): Query<EventQuery>,
) -> Response {
    match state.backend.poll_event(session_id, query.timeout_ms).await {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn reply_request_result(
    State(state): State<RestState>,
    Path((session_id, request_key)): Path<(String, String)>,
    body: std::result::Result<Json<RestRequestReplyResultRequest>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(error) => return invalid_json(error),
    };
    match state
        .backend
        .reply_request_result(session_id, request_key, body)
        .await
    {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

async fn reply_request_error(
    State(state): State<RestState>,
    Path((session_id, request_key)): Path<(String, String)>,
    body: std::result::Result<Json<RestErrorReplyRequest>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(error) => return invalid_json(error),
    };
    match state
        .backend
        .reply_request_error(session_id, request_key, body)
        .await
    {
        Ok(response) => Json(response).into_response(),
        Err(error) => rest_error(error),
    }
}

fn invalid_request(message: impl Into<String>) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(RestErrorResponse {
            error: "invalid_request".to_owned(),
            message: message.into(),
        }),
    )
        .into_response()
}

fn invalid_json(error: JsonRejection) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(RestErrorResponse {
            error: "invalid_json".to_owned(),
            message: error.to_string(),
        }),
    )
        .into_response()
}

fn rest_error(error: RestError) -> Response {
    match error {
        RestError::NotFound(message) => (
            StatusCode::NOT_FOUND,
            Json(RestErrorResponse {
                error: "not_found".to_owned(),
                message,
            }),
        )
            .into_response(),
        RestError::Client(error) => (
            StatusCode::BAD_GATEWAY,
            Json(RestErrorResponse {
                error: "codex_app_server_error".to_owned(),
                message: error.to_string(),
            }),
        )
            .into_response(),
    }
}

fn normalize_method(method: String) -> Option<String> {
    let method = method.trim_matches('/').trim();
    (!method.is_empty()).then(|| method.to_owned())
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

fn session_options_from(client: Option<RestClientOptions>, default_name: &str) -> SessionOptions {
    client
        .unwrap_or_default()
        .into_session_options(default_name)
}

async fn run_text_turn_with_handler<H>(
    session: &mut CodexSession,
    model: Option<String>,
    prompt: String,
    handler: &H,
) -> Result<TextTurnResult>
where
    H: crate::ApprovalHandler,
{
    let thread = if let Some(model) = model {
        session.start_thread_with_model(model).await?
    } else {
        session
            .start_thread(crate::protocol::ThreadStartParams::new())
            .await?
    };
    let turn = session.send_text_turn(&thread.thread.id, prompt).await?;
    let events = session
        .wait_for_turn_completed(&thread.thread.id, &turn.turn.id, handler)
        .await?;
    Ok(TextTurnResult {
        thread,
        turn,
        events,
    })
}
