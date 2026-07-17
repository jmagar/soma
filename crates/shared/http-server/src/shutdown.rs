//! Graceful shutdown signal.
//!
//! Resolves on `Ctrl+C` or, on Unix, `SIGTERM` — the two signals a process
//! manager (systemd, Docker, Kubernetes) or an interactive terminal sends to
//! ask a server to stop accepting new work and drain in-flight requests.

/// Waits for a shutdown signal (`Ctrl+C`, or `SIGTERM` on Unix).
///
/// Pass the resulting future to [`crate::serve_with_shutdown`] (or directly
/// to `axum::serve(..).with_graceful_shutdown(..)`) to drain in-flight
/// requests before the process exits.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::error!(%error, "CTRL+C handler failed");
            std::future::pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::error!(%error, "SIGTERM handler failed");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {}
        () = terminate => {}
    }
    tracing::info!("shutdown signal received");
}
