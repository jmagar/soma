//! `/v1/palette/*` route wiring.
//!
//! Handlers are thin: parse the HTTP input, call into `catalog`/`search`/
//! `schema`/`execute`, and translate the result into a response. All product
//! logic lives in those sibling modules, not here.

use axum::{
    extract::{rejection::JsonRejection, Extension, Query, State},
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};

use crate::{
    auth::{palette_execution_context, AuthContext},
    catalog::catalog_response,
    dto::{
        LauncherExecuteRequest, LauncherSchemaQuery, LauncherSearchQuery, LauncherSearchResponse,
    },
    error::{launcher_not_found, palette_error_response},
    execute::{execute_launcher, ExecuteOutcome},
    schema::find_schema,
    search::search_entries,
    state::PaletteState,
};

pub fn router() -> Router<PaletteState> {
    Router::new()
        .route("/v1/palette/catalog", get(get_catalog))
        .route("/v1/palette/search", get(get_search))
        .route("/v1/palette/schema", get(get_schema))
        .route("/v1/palette/execute", post(post_execute))
}

async fn get_catalog(State(state): State<PaletteState>) -> Response {
    let snapshot = state.application().catalog_snapshot();
    Json(catalog_response(&snapshot)).into_response()
}

async fn get_search(
    State(state): State<PaletteState>,
    Query(query): Query<LauncherSearchQuery>,
) -> Response {
    let snapshot = state.application().catalog_snapshot();
    let entries = crate::catalog::palette_entries(&snapshot);
    let results = search_entries(&entries, &query.q, query.limit);
    Json(LauncherSearchResponse { entries: results }).into_response()
}

async fn get_schema(
    State(state): State<PaletteState>,
    Query(query): Query<LauncherSchemaQuery>,
) -> Response {
    let snapshot = state.application().catalog_snapshot();
    match find_schema(&snapshot, &query.id) {
        Some(schema) => Json(schema).into_response(),
        None => launcher_not_found(&query.id),
    }
}

async fn post_execute(
    State(state): State<PaletteState>,
    auth: Option<Extension<AuthContext>>,
    body: Result<Json<LauncherExecuteRequest>, JsonRejection>,
) -> Response {
    let Json(request) = match body {
        Ok(body) => body,
        Err(error) => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": error.to_string()})),
            )
                .into_response()
        }
    };
    let context = palette_execution_context(&state, auth.as_ref().map(|Extension(auth)| auth));
    let id = request.id.clone();

    match execute_launcher(&state, request, context).await {
        ExecuteOutcome::Ok(response) => Json(response).into_response(),
        ExecuteOutcome::NotFound => launcher_not_found(&id),
        ExecuteOutcome::Failed(error) => palette_error_response(error),
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
