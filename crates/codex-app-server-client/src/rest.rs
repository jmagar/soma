use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::{Duration, Instant},
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
use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};

use crate::{
    protocol::ThreadStartParams, AllowAllApprovalHandler, CodexAppServerClient, CodexSession,
    CompatibilityReport, DenyAllApprovalHandler, Error, Event, PendingServerRequest,
    ReadOnlyApprovalHandler, SessionOptions, TextTurnResult,
};

pub type RestResult<T> = std::result::Result<T, RestError>;
pub type RestFuture<T> = Pin<Box<dyn Future<Output = RestResult<T>> + Send + 'static>>;

/// REST router behavior knobs.
///
/// [`Default`] is intentionally conservative: health, compatibility, and the
/// text-turn helper are mounted, but the raw bridge/session routes and
/// client-controlled unsafe options are not. Use [`Self::trusted_bridge`] only
/// when the caller mounts the router behind its own authz boundary.
#[derive(Clone, Debug, Default)]
pub struct RestRouterOptions {
    pub enable_bridge_routes: bool,
    pub allow_unsafe_client_options: bool,
    pub limits: RestLimits,
}

impl RestRouterOptions {
    /// Enables the full raw callable bridge for trusted deployments.
    pub fn trusted_bridge() -> Self {
        Self {
            enable_bridge_routes: true,
            allow_unsafe_client_options: true,
            limits: RestLimits::default(),
        }
    }

    pub fn with_max_sessions(mut self, max_sessions: usize) -> Self {
        self.limits.max_sessions = max_sessions;
        self
    }

    pub fn with_max_poll_timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.limits.max_poll_timeout = Duration::from_millis(timeout_ms);
        self
    }
}

/// Resource limits used by the default REST backend and route layer.
#[derive(Clone, Debug)]
pub struct RestLimits {
    pub max_sessions: usize,
    pub max_one_shot_concurrency: usize,
    pub max_poll_timeout: Duration,
    pub pending_request_ttl: Duration,
    pub max_pending_requests_per_session: usize,
    pub idle_session_ttl: Duration,
    pub compatibility_ttl: Duration,
}

impl Default for RestLimits {
    fn default() -> Self {
        Self {
            max_sessions: 16,
            max_one_shot_concurrency: 4,
            max_poll_timeout: Duration::from_secs(30),
            pending_request_ttl: Duration::from_secs(600),
            max_pending_requests_per_session: 64,
            idle_session_ttl: Duration::from_secs(30 * 60),
            compatibility_ttl: Duration::from_secs(30),
        }
    }
}

/// Errors surfaced by the optional REST adapter.
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    #[error("{0}")]
    NotFound(String),

    #[error("{0}")]
    Gone(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    RateLimited(String),

    #[error("{0}")]
    Conflict(String),

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
#[derive(Clone)]
pub struct CodexRestBackend {
    sessions: Arc<CodexRestSessions>,
    limits: RestLimits,
    compatibility: Arc<StdMutex<Option<CachedCompatibility>>>,
}

impl Default for CodexRestBackend {
    fn default() -> Self {
        Self::with_limits(RestLimits::default())
    }
}

#[derive(Default)]
struct CodexRestSessions {
    next_session_id: AtomicU64,
    sessions: Mutex<HashMap<String, Arc<CodexRestSession>>>,
}

struct CodexRestSession {
    client: CodexAppServerClient,
    session: Mutex<CodexSession>,
    pending_requests: Mutex<HashMap<String, PendingRestRequest>>,
    next_request_key: AtomicU64,
    last_used: Mutex<Instant>,
}

struct PendingRestRequest {
    request: PendingServerRequest,
    expires_at: Instant,
}

struct CachedCompatibility {
    report: CompatibilityReport,
    expires_at: Instant,
}

impl CodexRestBackend {
    pub fn with_limits(limits: RestLimits) -> Self {
        Self {
            sessions: Arc::default(),
            limits,
            compatibility: Arc::default(),
        }
    }

    async fn session(&self, session_id: &str) -> RestResult<Arc<CodexRestSession>> {
        self.prune_idle_sessions().await;
        let session = self
            .sessions
            .sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| RestError::NotFound(format!("session `{session_id}` was not found")))?;
        session.touch().await;
        Ok(session)
    }

    async fn prune_idle_sessions(&self) {
        let now = Instant::now();
        let entries = {
            let sessions = self.sessions.sessions.lock().await;
            sessions
                .iter()
                .map(|(id, session)| (id.clone(), session.clone()))
                .collect::<Vec<_>>()
        };

        let mut expired = Vec::new();
        for (id, session) in entries {
            let last_used = *session.last_used.lock().await;
            if now.duration_since(last_used) >= self.limits.idle_session_ttl {
                expired.push(id);
            }
        }

        if expired.is_empty() {
            return;
        }

        let mut sessions = self.sessions.sessions.lock().await;
        for id in expired {
            sessions.remove(&id);
        }
    }
}

impl CodexRestSession {
    async fn touch(&self) {
        *self.last_used.lock().await = Instant::now();
    }

    async fn prune_expired_pending(&self, now: Instant) {
        prune_expired_pending_requests(&self.pending_requests, now).await;
    }

    async fn take_pending_request(&self, request_key: &str) -> RestResult<PendingServerRequest> {
        take_pending_request(&self.pending_requests, request_key).await
    }
}

async fn prune_expired_pending_requests(
    pending_requests: &Mutex<HashMap<String, PendingRestRequest>>,
    now: Instant,
) {
    let mut pending = pending_requests.lock().await;
    let expired = pending
        .iter()
        .filter(|(_, request)| request.expires_at <= now)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in expired {
        pending.remove(&key);
    }
}

async fn take_pending_request(
    pending_requests: &Mutex<HashMap<String, PendingRestRequest>>,
    request_key: &str,
) -> RestResult<PendingServerRequest> {
    let now = Instant::now();
    let mut pending = pending_requests.lock().await;
    if pending
        .get(request_key)
        .is_some_and(|request| request.expires_at <= now)
    {
        pending.remove(request_key);
        return Err(RestError::Gone(format!(
            "request `{request_key}` has expired"
        )));
    }

    let expired = pending
        .iter()
        .filter(|(_, request)| request.expires_at <= now)
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in expired {
        pending.remove(&key);
    }

    pending
        .remove(request_key)
        .map(|request| request.request)
        .ok_or_else(|| RestError::NotFound(format!("request `{request_key}` was not found")))
}

impl RestBackend for CodexRestBackend {
    fn compatibility_report(&self) -> CompatibilityReport {
        let now = Instant::now();
        let mut cached = self
            .compatibility
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cached) = cached.as_ref() {
            if cached.expires_at > now {
                return cached.report.clone();
            }
        }

        let report = CompatibilityReport::current();
        *cached = Some(CachedCompatibility {
            report: report.clone(),
            expires_at: now + self.limits.compatibility_ttl,
        });
        report
    }

    fn run_text_turn(&self, request: RestTextTurnRequest) -> RestFuture<RestTextTurnResponse> {
        Box::pin(async move {
            let session_options = request.session_options();
            let approval_policy = request.approval_policy.unwrap_or_default();
            let thread_params = request
                .model
                .clone()
                .map_or_else(ThreadStartParams::new, |model| {
                    ThreadStartParams::new().model(model)
                });
            let prompt = request.prompt.clone();
            let mut session = CodexSession::spawn(session_options).await?;

            let result = match approval_policy {
                RestApprovalPolicy::DenyAll => {
                    session
                        .run_text_turn_with_params_and_handler(
                            thread_params,
                            prompt,
                            &DenyAllApprovalHandler::default(),
                        )
                        .await?
                }
                RestApprovalPolicy::ReadOnly => {
                    session
                        .run_text_turn_with_params_and_handler(
                            thread_params,
                            prompt,
                            &ReadOnlyApprovalHandler,
                        )
                        .await?
                }
                RestApprovalPolicy::AllowAll => {
                    session
                        .run_text_turn_with_params_and_handler(
                            thread_params,
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
            backend.prune_idle_sessions().await;
            if backend.sessions.sessions.lock().await.len() >= backend.limits.max_sessions {
                return Err(RestError::RateLimited(format!(
                    "maximum REST session count ({}) reached",
                    backend.limits.max_sessions
                )));
            }

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
                last_used: Mutex::new(Instant::now()),
            });
            let mut sessions = backend.sessions.sessions.lock().await;
            if sessions.len() >= backend.limits.max_sessions {
                return Err(RestError::RateLimited(format!(
                    "maximum REST session count ({}) reached",
                    backend.limits.max_sessions
                )));
            }
            sessions.insert(session_id.clone(), rest_session);
            Ok(RestSessionCreateResponse {
                session_id,
                initialize_response,
            })
        })
    }

    fn list_sessions(&self) -> RestFuture<Vec<RestSessionSummary>> {
        let backend = self.clone();
        Box::pin(async move {
            backend.prune_idle_sessions().await;
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
        let limits = self.limits.clone();
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
                    let now = Instant::now();
                    session.prune_expired_pending(now).await;
                    let request_key = format!(
                        "request-{}",
                        session.next_request_key.fetch_add(1, Ordering::Relaxed) + 1
                    );
                    let request_id = serde_json::to_value(request.id())?;
                    let method = request.method_name().to_owned();
                    let request_value = serde_json::to_value(&request.request)?;
                    let mut pending = session.pending_requests.lock().await;
                    if pending.len() >= limits.max_pending_requests_per_session {
                        request.respond_error(-32000, "REST pending request limit reached", None);
                        return Err(RestError::RateLimited(format!(
                            "maximum pending request count ({}) reached",
                            limits.max_pending_requests_per_session
                        )));
                    }
                    pending.insert(
                        request_key.clone(),
                        PendingRestRequest {
                            request,
                            expires_at: now + limits.pending_request_ttl,
                        },
                    );
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
            let pending = session.take_pending_request(&request_key).await?;
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
            let pending = session.take_pending_request(&request_key).await?;
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
    options: RestRouterOptions,
    one_shot_gate: Arc<Semaphore>,
    active_polls: Arc<StdMutex<HashSet<String>>>,
}

struct ActivePollGuard {
    active_polls: Arc<StdMutex<HashSet<String>>>,
    session_id: String,
}

impl Drop for ActivePollGuard {
    fn drop(&mut self) {
        let mut active = self
            .active_polls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        active.remove(&self.session_id);
    }
}

/// Builds a conservative REST router backed by real `codex app-server` processes.
///
/// The default router exposes health, compatibility, and the text-turn helper.
/// It does not mount the raw callable bridge/session routes. Use
/// [`trusted_bridge_router`] or [`router_with_options`] when the router is
/// mounted behind a trusted authz boundary and should expose the full bridge.
pub fn router() -> Router {
    router_with_options(RestRouterOptions::default())
}

/// Builds a trusted full bridge router backed by real `codex app-server` processes.
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
pub fn trusted_bridge_router() -> Router {
    router_with_options(RestRouterOptions::trusted_bridge())
}

/// Builds a REST router backed by real `codex app-server` processes and options.
pub fn router_with_options(options: RestRouterOptions) -> Router {
    router_with_backend_and_options(
        CodexRestBackend::with_limits(options.limits.clone()),
        options,
    )
}

/// Builds a REST router with a caller-provided backend.
pub fn router_with_backend<B>(backend: B) -> Router
where
    B: RestBackend,
{
    router_with_backend_and_options(backend, RestRouterOptions::default())
}

/// Builds a REST router with a caller-provided backend and options.
pub fn router_with_backend_and_options<B>(backend: B, options: RestRouterOptions) -> Router
where
    B: RestBackend,
{
    router_with_backend_arc_and_options(Arc::new(backend), options)
}

/// Builds a REST router from a shared backend trait object.
pub fn router_with_backend_arc(backend: Arc<dyn RestBackend>) -> Router {
    router_with_backend_arc_and_options(backend, RestRouterOptions::default())
}

/// Builds a REST router from a shared backend trait object and options.
pub fn router_with_backend_arc_and_options(
    backend: Arc<dyn RestBackend>,
    options: RestRouterOptions,
) -> Router {
    let state = RestState {
        backend,
        one_shot_gate: Arc::new(Semaphore::new(options.limits.max_one_shot_concurrency)),
        active_polls: Arc::default(),
        options: options.clone(),
    };

    let router = Router::new()
        .route("/health", get(health))
        .route("/v1/health", get(health))
        .route("/v1/compatibility", get(compatibility))
        .route("/v1/text-turn", post(text_turn));

    let router = if options.enable_bridge_routes {
        router
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
    } else {
        router
    };

    router.with_state(state)
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
    pub turn_status: Option<String>,
    pub agent_message: String,
    pub latest_diff: Option<String>,
    pub errors: Vec<Value>,
}

impl From<TextTurnResult> for RestTextTurnResponse {
    fn from(result: TextTurnResult) -> Self {
        let agent_message = result.agent_message().to_owned();
        let latest_diff = result.latest_diff().map(str::to_owned);
        let turn_status = result.events.terminal_status().and_then(|status| {
            serde_json::to_value(status)
                .ok()?
                .as_str()
                .map(str::to_owned)
        });
        let errors = result
            .errors()
            .iter()
            .map(|error| {
                serde_json::to_value(error).unwrap_or_else(|_| Value::String(error.message.clone()))
            })
            .collect();
        Self {
            thread_id: result.thread.thread.id,
            turn_id: result.turn.turn.id,
            turn_status,
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
    #[serde(default)]
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
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RestErrorResponse {
    pub error: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
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
    if let Err(error) = validate_text_turn_request(&state.options, &request) {
        return rest_error(error);
    }

    let _permit = match acquire_one_shot_permit(&state) {
        Ok(permit) => permit,
        Err(error) => return rest_error(error),
    };
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
    if let Err(error) = validate_client_options(&state.options, body.client.as_ref()) {
        return rest_error(error);
    }
    let _permit = match acquire_one_shot_permit(&state) {
        Ok(permit) => permit,
        Err(error) => return rest_error(error),
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
    if let Err(error) = validate_client_options(&state.options, request.client.as_ref()) {
        return rest_error(error);
    }
    match state.backend.list_sessions().await {
        Ok(sessions) if sessions.len() >= state.options.limits.max_sessions => {
            return rest_error(RestError::RateLimited(format!(
                "maximum REST session count ({}) reached",
                state.options.limits.max_sessions
            )));
        }
        Ok(_) => {}
        Err(error) => return rest_error(error),
    }
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
    if let Err(error) = validate_client_options(&state.options, body.client.as_ref()) {
        return rest_error(error);
    }
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
    let _guard = match acquire_poll_guard(&state, &session_id) {
        Ok(guard) => guard,
        Err(error) => return rest_error(error),
    };
    let timeout_ms = Some(clamp_poll_timeout_ms(
        &state.options,
        query
            .timeout_ms
            .unwrap_or(state.options.limits.max_poll_timeout.as_millis() as u64),
    ));
    match state.backend.poll_event(session_id, timeout_ms).await {
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
            code: None,
            data: None,
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
            code: None,
            data: None,
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
                code: None,
                data: None,
            }),
        )
            .into_response(),
        RestError::Gone(message) => (
            StatusCode::GONE,
            Json(RestErrorResponse {
                error: "gone".to_owned(),
                message,
                code: None,
                data: None,
            }),
        )
            .into_response(),
        RestError::Forbidden(message) => (
            StatusCode::FORBIDDEN,
            Json(RestErrorResponse {
                error: "forbidden".to_owned(),
                message,
                code: None,
                data: None,
            }),
        )
            .into_response(),
        RestError::RateLimited(message) => (
            StatusCode::TOO_MANY_REQUESTS,
            Json(RestErrorResponse {
                error: "rate_limited".to_owned(),
                message,
                code: None,
                data: None,
            }),
        )
            .into_response(),
        RestError::Conflict(message) => (
            StatusCode::CONFLICT,
            Json(RestErrorResponse {
                error: "conflict".to_owned(),
                message,
                code: None,
                data: None,
            }),
        )
            .into_response(),
        RestError::Client(Error::Rpc {
            code,
            message,
            data,
        }) => (
            StatusCode::BAD_GATEWAY,
            Json(RestErrorResponse {
                error: "json_rpc_error".to_owned(),
                message,
                code: Some(code),
                data,
            }),
        )
            .into_response(),
        RestError::Client(error) => (
            StatusCode::BAD_GATEWAY,
            Json(RestErrorResponse {
                error: "codex_app_server_error".to_owned(),
                message: error.to_string(),
                code: None,
                data: None,
            }),
        )
            .into_response(),
    }
}

fn validate_text_turn_request(
    options: &RestRouterOptions,
    request: &RestTextTurnRequest,
) -> RestResult<()> {
    if !options.allow_unsafe_client_options
        && matches!(request.approval_policy, Some(RestApprovalPolicy::AllowAll))
    {
        return Err(RestError::Forbidden(
            "`approvalPolicy: allow_all` requires a trusted REST bridge".to_owned(),
        ));
    }
    validate_client_options(options, request.client.as_ref())
}

fn validate_client_options(
    options: &RestRouterOptions,
    client: Option<&RestClientOptions>,
) -> RestResult<()> {
    if options.allow_unsafe_client_options {
        return Ok(());
    }
    let Some(client) = client else {
        return Ok(());
    };
    if client.command.is_some() || !client.extra_args.is_empty() || !client.config.is_empty() {
        return Err(RestError::Forbidden(
            "client command, extraArgs, and config overrides require a trusted REST bridge"
                .to_owned(),
        ));
    }
    Ok(())
}

fn acquire_one_shot_permit(state: &RestState) -> RestResult<OwnedSemaphorePermit> {
    state
        .one_shot_gate
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            RestError::RateLimited(format!(
                "maximum one-shot REST call concurrency ({}) reached",
                state.options.limits.max_one_shot_concurrency
            ))
        })
}

fn acquire_poll_guard(state: &RestState, session_id: &str) -> RestResult<ActivePollGuard> {
    let mut active = state
        .active_polls
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if !active.insert(session_id.to_owned()) {
        return Err(RestError::Conflict(format!(
            "an event poll is already active for session `{session_id}`"
        )));
    }
    Ok(ActivePollGuard {
        active_polls: state.active_polls.clone(),
        session_id: session_id.to_owned(),
    })
}

fn clamp_poll_timeout_ms(options: &RestRouterOptions, timeout_ms: u64) -> u64 {
    let max = options.limits.max_poll_timeout.as_millis() as u64;
    timeout_ms.min(max)
}

fn normalize_method(method: String) -> Option<String> {
    let method = method.trim_matches('/').trim();
    (!method.is_empty()).then(|| method.to_owned())
}

fn session_options_from(client: Option<RestClientOptions>, default_name: &str) -> SessionOptions {
    client
        .unwrap_or_default()
        .into_session_options(default_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{CurrentTimeReadParams, RequestId, ServerRequest};

    #[tokio::test]
    async fn expired_pending_request_returns_gone_and_removes_the_key() {
        let pending_requests = Mutex::new(HashMap::from([(
            "request-1".to_owned(),
            PendingRestRequest {
                request: pending_current_time_request(),
                expires_at: Instant::now() - Duration::from_secs(1),
            },
        )]));

        let err = take_pending_request(&pending_requests, "request-1")
            .await
            .expect_err("expired request key should be rejected");

        assert!(matches!(err, RestError::Gone(_)));
        assert!(pending_requests.lock().await.is_empty());
    }

    #[tokio::test]
    async fn taking_pending_request_prunes_other_expired_keys() {
        let pending_requests = Mutex::new(HashMap::from([
            (
                "expired".to_owned(),
                PendingRestRequest {
                    request: pending_current_time_request(),
                    expires_at: Instant::now() - Duration::from_secs(1),
                },
            ),
            (
                "fresh".to_owned(),
                PendingRestRequest {
                    request: pending_current_time_request(),
                    expires_at: Instant::now() + Duration::from_secs(60),
                },
            ),
        ]));

        let request = take_pending_request(&pending_requests, "fresh")
            .await
            .expect("fresh request key should be returned");

        assert_eq!(request.method_name(), "currentTime/read");
        assert!(pending_requests.lock().await.is_empty());
    }

    fn pending_current_time_request() -> PendingServerRequest {
        PendingServerRequest::for_test(ServerRequest::CurrentTimeRead {
            id: RequestId::Int64(7),
            params: CurrentTimeReadParams {
                thread_id: "thread-test".to_owned(),
            },
        })
    }
}
