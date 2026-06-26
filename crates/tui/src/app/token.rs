//! The redacting holder for the session bearer JWT.
//!
//! The JWT is sensitive — anything reachable from a derived `Debug` (a log line, a `tracing`
//! span field, a panic message, auto-instrumentation) would leak it. [`SessionToken`] wraps the
//! bearer string with a hand-written [`fmt::Debug`] that renders `[REDACTED]`, mirroring
//! [`contract::Password`], so the token can be carried by `#[derive(Debug)]` structs and enums
//! without ever printing. Expose the inner value only at the point of use (the request worker
//! attaching the `Authorization: Bearer` header) with [`SessionToken::expose`].

use std::fmt;

/// The session bearer JWT, held in memory for the process lifetime only (hard-constraint #1).
///
/// Its [`fmt::Debug`] renders `[REDACTED]`, so the secret never leaks through a derived `Debug`,
/// a log line, a trace span, or auto-instrumentation. Expose the inner string only at the point
/// of use with [`SessionToken::expose`].
///
/// # Examples
///
/// ```
/// use tui::app::SessionToken;
///
/// let token = SessionToken::new("jwt.abc.123".to_owned());
/// // The bearer string is readable at the point of use...
/// assert_eq!(token.expose(), "jwt.abc.123");
/// // ...but never leaks through Debug.
/// assert_eq!(format!("{token:?}"), "[REDACTED]");
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    /// Wraps a bearer JWT string.
    #[must_use]
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrows the underlying bearer string. Call this only at the point the secret is used
    /// (attaching the `Authorization: Bearer` header); never log or store the returned value.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for SessionToken {
    fn from(value: String) -> Self {
        Self(value)
    }
}
