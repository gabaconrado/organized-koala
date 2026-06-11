//! Auth wire types: registration, login, and the session token they return.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A plaintext password as it crosses the wire.
///
/// Serializes and deserializes transparently as a JSON string (so the TUI can send it and
/// the server can read it), but its [`fmt::Debug`] renders as `[REDACTED]` so the secret can
/// never leak through a derived `Debug`, a log line, a trace span, or auto-instrumentation.
/// Expose the inner value only at the point of use with [`Password::expose`].
///
/// # Examples
///
/// ```
/// use contract::RegisterRequest;
///
/// let req = serde_json::from_str::<RegisterRequest>(
///     r#"{"username":"ada","email":"ada@example.com","password":"hunter2","profile_name":"work"}"#,
/// )
/// .unwrap();
///
/// // The password is readable at the point of use...
/// assert_eq!(req.password.expose(), "hunter2");
/// // ...but never leaks through Debug.
/// assert_eq!(format!("{:?}", req.password), "[REDACTED]");
/// ```
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Password(String);

impl Password {
    /// Wraps a plaintext password.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrows the underlying plaintext. Call this only at the point the secret is used
    /// (e.g. hashing on the server); never log or store the returned value.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for Password {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Request body for `POST /api/auth/register`.
///
/// Registration is atomic: the server creates the user and their default profile (named
/// `profile_name`) in one transaction, so a user without a profile cannot exist. The
/// `username` must not contain `@` and the `email` must be a valid email — these rules are
/// enforced server-side; the contract only carries the shape. On success the server returns
/// `201` with a [`SessionResponse`].
///
/// # Examples
///
/// ```
/// use contract::{Password, RegisterRequest};
///
/// let req = RegisterRequest {
///     username: "ada".to_owned(),
///     email: "ada@example.com".to_owned(),
///     password: Password::new("hunter2".to_owned()),
///     profile_name: "work".to_owned(),
/// };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["username"], "ada");
/// assert_eq!(json["profile_name"], "work");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    /// Login username. Must not contain `@` (enforced server-side).
    pub username: String,
    /// Login email. Must be a valid email address (enforced server-side).
    pub email: String,
    /// Plaintext password; hashed with argon2 server-side, never stored as-is.
    pub password: Password,
    /// Name of the default profile created atomically with the account.
    pub profile_name: String,
}

/// Request body for `POST /api/auth/login`.
///
/// `identifier` matches either the username or the email. On success the server returns
/// `200` with a [`SessionResponse`]; a mismatch is `401 invalid_credentials`.
///
/// # Examples
///
/// ```
/// use contract::{LoginRequest, Password};
///
/// let req = LoginRequest {
///     identifier: "ada@example.com".to_owned(),
///     password: Password::new("hunter2".to_owned()),
/// };
/// let json = serde_json::to_value(&req).unwrap();
/// assert_eq!(json["identifier"], "ada@example.com");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Username or email identifying the account.
    pub identifier: String,
    /// Plaintext password, verified against the stored argon2 hash server-side.
    pub password: Password,
}

/// Response body returned by both register (`201`) and login (`200`).
///
/// Carries the JWT the client sends back as `Authorization: Bearer <token>`. The TUI holds
/// it in memory only; expiry surfaces later as `401 unauthenticated`.
///
/// # Examples
///
/// ```
/// use contract::SessionResponse;
///
/// let session = serde_json::from_str::<SessionResponse>(r#"{"token":"jwt.abc.123"}"#).unwrap();
/// assert_eq!(session.token, "jwt.abc.123");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResponse {
    /// The session JWT (HS256). Sent back as a `Bearer` token on authenticated requests.
    pub token: String,
}
