//! Liveness probe (ADR-0005 §7): unauthenticated, trivial body.

use axum::http::StatusCode;

/// `GET /healthz` → `200` with an empty body. Used by compose healthchecks and the TUI's
/// "is the server online" probe.
#[tracing::instrument]
pub async fn healthz() -> StatusCode {
    StatusCode::OK
}
