//! The axum application: shared state and the route table.

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use sqlx::PgPool;

use crate::auth::Jwt;
use crate::config::JwtConfig;
use crate::handlers;

/// Shared, cheaply-cloneable handler state: the DB pool and the session issuer/verifier.
#[derive(Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

struct Inner {
    pool: PgPool,
    jwt: Jwt,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState").finish_non_exhaustive()
    }
}

impl AppState {
    /// Build state from a connection pool and the JWT configuration.
    pub fn new(pool: PgPool, jwt: JwtConfig) -> Self {
        Self {
            inner: Arc::new(Inner {
                pool,
                jwt: Jwt::new(jwt.secret, jwt.ttl),
            }),
        }
    }

    /// The connection pool.
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// The session issuer/verifier.
    pub fn jwt(&self) -> &Jwt {
        &self.inner.jwt
    }
}

/// Build the full route table (ADR-0005). `/healthz` is unauthenticated; the `/api/profiles`
/// routes require a valid session and are profile-scoped by ownership in the handlers.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(handlers::healthz))
        .route("/api/auth/register", post(handlers::register))
        .route("/api/auth/login", post(handlers::login))
        .route("/api/profiles", get(handlers::list_profiles))
        .route(
            "/api/profiles/{profile_id}/tasks",
            get(handlers::list_tasks).post(handlers::create_task),
        )
        .route(
            "/api/profiles/{profile_id}/tasks/{task_id}/close",
            post(handlers::close_task),
        )
        .route(
            "/api/timer/config",
            get(handlers::get_config).put(handlers::update_config),
        )
        .route("/api/timer/session", get(handlers::get_session))
        .route("/api/timer/session/start", post(handlers::start_session))
        .route("/api/timer/session/stop", post(handlers::stop_session))
        .with_state(state)
}
