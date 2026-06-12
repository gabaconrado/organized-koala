//! JWT HS256 session issue + verify (ADR-0005 §3). The signing secret is held as a
//! [`SecretString`] and exposed only when (de)coding a token, so it never reaches a
//! `Debug`/log/span.

use std::time::Duration;

use anyhow::Context as _;
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use secrecy::{ExposeSecret as _, SecretString};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// JWT claims: subject (user UUID), issued-at, and expiry (ADR-0005 §3).
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// Subject: the user's UUID.
    sub: String,
    /// Issued-at (Unix seconds).
    iat: i64,
    /// Expiry (Unix seconds); enforced on verification.
    exp: i64,
}

/// Issues and verifies HS256 session tokens. Holds the signing secret redacted.
pub struct Jwt {
    secret: SecretString,
    ttl: Duration,
}

impl std::fmt::Debug for Jwt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jwt")
            .field("secret", &"[REDACTED]")
            .field("ttl", &self.ttl)
            .finish()
    }
}

impl Jwt {
    /// Build a session issuer/verifier from the signing secret and token TTL.
    pub fn new(secret: SecretString, ttl: Duration) -> Self {
        Self { secret, ttl }
    }

    /// Issue a signed token for `user_id`, expiring after the configured TTL.
    pub fn issue(&self, user_id: Uuid) -> anyhow::Result<String> {
        let now = Utc::now();
        let ttl = chrono::Duration::from_std(self.ttl).context("JWT TTL out of range")?;
        let claims = Claims {
            sub: user_id.to_string(),
            iat: now.timestamp(),
            exp: (now + ttl).timestamp(),
        };
        let key = EncodingKey::from_secret(self.secret.expose_secret().as_bytes());
        encode(&Header::new(Algorithm::HS256), &claims, &key).context("signing the session token")
    }

    /// Verify a token and return the authenticated user id. `exp` is enforced.
    ///
    /// A malformed signature, an expired token, or a non-UUID subject all map to `None`; the
    /// caller surfaces this as `401 unauthenticated`.
    pub fn verify(&self, token: &str) -> Option<Uuid> {
        let key = DecodingKey::from_secret(self.secret.expose_secret().as_bytes());
        let validation = Validation::new(Algorithm::HS256);
        let claims = decode::<Claims>(token, &key, &validation).ok()?.claims;
        Uuid::parse_str(&claims.sub).ok()
    }
}
