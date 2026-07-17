//! Generic per-request timeout.
//!
//! Unlike plain `tower::timeout`, `tower_http`'s [`TimeoutLayer`] never
//! fails the inner service — on elapse it directly returns a response with
//! the configured status code (`408 Request Timeout` by default), so it
//! composes onto an Axum `Router` with no error-handling layer required.

use std::time::Duration;

use axum::http::StatusCode;
pub use tower_http::timeout::TimeoutLayer;

/// Build a layer that returns `408 Request Timeout` for any request taking
/// longer than `duration`.
pub fn timeout_layer(duration: Duration) -> TimeoutLayer {
    TimeoutLayer::with_status_code(StatusCode::REQUEST_TIMEOUT, duration)
}

/// Build a layer that returns `status_code` for any request taking longer
/// than `duration`.
pub fn timeout_layer_with_status(status_code: StatusCode, duration: Duration) -> TimeoutLayer {
    TimeoutLayer::with_status_code(status_code, duration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, routing::get, Router};
    use tower::ServiceExt;

    #[tokio::test]
    async fn fast_handler_is_unaffected_by_a_generous_timeout() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(timeout_layer(Duration::from_secs(30)));

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn slow_handler_is_cut_off_by_the_timeout() {
        let app = Router::new()
            .route(
                "/",
                get(|| async {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    "too slow"
                }),
            )
            .layer(timeout_layer(Duration::from_millis(20)));

        let request = Request::builder().uri("/").body(Body::empty()).unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);
    }
}
