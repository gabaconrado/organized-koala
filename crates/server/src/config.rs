//! Server configuration sourced from the environment. Secrets are held as [`SecretString`]
//! so they never surface through `Debug`, a log line, or a span field.

use std::time::Duration;

use anyhow::Context as _;
use secrecy::SecretString;

/// Default session token lifetime: 24 hours (ADR-0005 §3).
const DEFAULT_JWT_TTL_SECONDS: u64 = 86_400;
/// Default HTTP listen address.
const DEFAULT_BIND_ADDR: &str = "0.0.0.0:8080";

/// JWT signing configuration. `secret` is redacted in any `Debug`/log output.
pub struct JwtConfig {
    /// HS256 signing secret.
    pub secret: SecretString,
    /// Token time-to-live.
    pub ttl: Duration,
}

impl std::fmt::Debug for JwtConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtConfig")
            .field("secret", &"[REDACTED]")
            .field("ttl", &self.ttl)
            .finish()
    }
}

/// Resolved configuration for `organized-koalad run`.
#[derive(Debug)]
pub struct Config {
    /// Postgres connection string.
    pub database_url: String,
    /// JWT signing configuration.
    pub jwt: JwtConfig,
    /// HTTP listen address.
    pub bind_addr: String,
    /// OTLP collector endpoint; tracing exports there when present.
    pub otlp_endpoint: Option<String>,
    /// Dev-only hatch: apply pending migrations on the run path when set.
    pub auto_migrate: bool,
}

impl Config {
    /// Load configuration from the environment, applying documented defaults.
    ///
    /// `OK_DATABASE_URL` and `OK_JWT_SECRET` are required; everything else has a default.
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = database_url_from_env()?;

        let secret = SecretString::from(
            require_env("OK_JWT_SECRET").context("a JWT signing secret is required for `run`")?,
        );
        let ttl = match std::env::var("OK_JWT_TTL_SECONDS") {
            Ok(raw) => Duration::from_secs(
                raw.parse()
                    .context("OK_JWT_TTL_SECONDS must be a non-negative integer")?,
            ),
            Err(_) => Duration::from_secs(DEFAULT_JWT_TTL_SECONDS),
        };

        let bind_addr =
            std::env::var("OK_BIND_ADDR").unwrap_or_else(|_| DEFAULT_BIND_ADDR.to_owned());
        let otlp_endpoint = std::env::var("OK_OTLP_ENDPOINT")
            .ok()
            .filter(|v| !v.is_empty());
        let auto_migrate = matches!(std::env::var("OK_AUTO_MIGRATE").as_deref(), Ok("1"));

        Ok(Self {
            database_url,
            jwt: JwtConfig { secret, ttl },
            bind_addr,
            otlp_endpoint,
            auto_migrate,
        })
    }
}

/// Read the required `OK_DATABASE_URL`. Shared by `run`, `migrate`, and `rollback`.
pub fn database_url_from_env() -> anyhow::Result<String> {
    require_env("OK_DATABASE_URL").context("a Postgres connection string is required")
}

/// Read a required environment variable, erroring with its name when absent or empty.
fn require_env(name: &str) -> anyhow::Result<String> {
    match std::env::var(name) {
        Ok(value) if !value.is_empty() => Ok(value),
        _ => anyhow::bail!("{name} must be set"),
    }
}
