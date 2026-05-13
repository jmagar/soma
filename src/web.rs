//! Static web asset serving — embeds apps/web/out/ into the binary at compile time.
//!
//! When the `web` feature is enabled (default), `include_dir!` bakes every file from
//! `apps/web/out/` into the binary at compile time.  At runtime, `serve_web_assets`
//! responds to any HTTP request that wasn't matched by a more specific route (MCP, REST,
//! health, OAuth).
//!
//! The SPA fallback strategy:
//!   1. Serve the exact path if it exists          (e.g. `/favicon.ico`)
//!   2. Try path + `.html`                          (e.g. `/tools` → `tools.html`)
//!   3. Try path + `/index.html`                   (e.g. `/tools/` → `tools/index.html`)
//!   4. Fall back to `index.html` for client-side routing
//!
//! Cache-control:
//!   - `index.html`         → `no-store`  (SPA shell must never be stale)
//!   - All other assets     → `public, max-age=31536000, immutable` (content-hashed by Next.js)

#[cfg(feature = "web")]
use include_dir::{include_dir, Dir};

#[cfg(feature = "web")]
static WEB_ASSETS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/apps/web/out");

use axum::{
    body::Body,
    extract::Request,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

/// Returns `true` when the embedded `apps/web/out/index.html` is present.
///
/// Used by `AppState::web_assets_enabled()` and the router to decide whether to wire
/// up the SPA fallback handler.
pub fn web_assets_available() -> bool {
    #[cfg(feature = "web")]
    {
        WEB_ASSETS.get_file("index.html").is_some()
    }
    #[cfg(not(feature = "web"))]
    {
        false
    }
}

/// Axum fallback handler — serves embedded static assets with SPA fallback.
pub async fn serve_web_assets(request: Request<Body>) -> Response {
    #[cfg(feature = "web")]
    {
        let path = request.uri().path().trim_start_matches('/');

        // Ordered candidate list — first match wins.
        let candidates: &[String] = &[
            path.to_string(),
            format!("{path}.html"),
            format!("{path}/index.html"),
        ];

        for candidate in candidates {
            if let Some(file) = WEB_ASSETS.get_file(candidate.as_str()) {
                let content_type = guess_mime(candidate);
                let cache_control =
                    if candidate.ends_with("index.html") || candidate == "index.html" {
                        "no-store"
                    } else {
                        "public, max-age=31536000, immutable"
                    };
                return (
                    StatusCode::OK,
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::CACHE_CONTROL, cache_control),
                    ],
                    file.contents().to_vec(),
                )
                    .into_response();
            }
        }

        // SPA fallback — let client-side router handle the path
        if let Some(file) = WEB_ASSETS.get_file("index.html") {
            return (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, "text/html; charset=utf-8"),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                file.contents().to_vec(),
            )
                .into_response();
        }

        StatusCode::NOT_FOUND.into_response()
    }

    #[cfg(not(feature = "web"))]
    {
        let _ = request;
        StatusCode::NOT_FOUND.into_response()
    }
}

fn guess_mime(path: &str) -> &'static str {
    if path.ends_with(".html") {
        "text/html; charset=utf-8"
    } else if path.ends_with(".css") {
        "text/css; charset=utf-8"
    } else if path.ends_with(".js") || path.ends_with(".mjs") {
        "application/javascript; charset=utf-8"
    } else if path.ends_with(".json") {
        "application/json"
    } else if path.ends_with(".svg") {
        "image/svg+xml"
    } else if path.ends_with(".png") {
        "image/png"
    } else if path.ends_with(".jpg") || path.ends_with(".jpeg") {
        "image/jpeg"
    } else if path.ends_with(".ico") {
        "image/x-icon"
    } else if path.ends_with(".woff2") {
        "font/woff2"
    } else if path.ends_with(".woff") {
        "font/woff"
    } else if path.ends_with(".ttf") {
        "font/ttf"
    } else if path.ends_with(".txt") {
        "text/plain; charset=utf-8"
    } else if path.ends_with(".webmanifest") {
        "application/manifest+json"
    } else {
        "application/octet-stream"
    }
}
