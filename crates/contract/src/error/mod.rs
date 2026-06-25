//! The standard error payload and its stable, machine-matchable code identifiers.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A stable, machine-matchable error-code identifier (ADR-0005 §6).
///
/// On the wire a code is a lowercase string. Known codes deserialize to their named variant;
/// any unrecognized string is preserved in [`ErrorCode::Unknown`] rather than failing, so a
/// consumer built against an older code set still parses newer server responses (the code set
/// is append-only). Match on the named variants for known cases and treat
/// [`ErrorCode::Unknown`] as a generic error.
///
/// # Examples
///
/// ```
/// use contract::ErrorCode;
///
/// // Known codes round-trip to their variant.
/// let code: ErrorCode = serde_json::from_str(r#""not_found""#).unwrap();
/// assert_eq!(code, ErrorCode::NotFound);
/// assert_eq!(code.as_str(), "not_found");
///
/// let conflict: ErrorCode = serde_json::from_str(r#""profile_name_taken""#).unwrap();
/// assert_eq!(conflict, ErrorCode::ProfileNameTaken);
/// assert_eq!(ErrorCode::LastProfile.as_str(), "last_profile");
///
/// // Unknown codes are preserved, not rejected (forward-compatible).
/// let future: ErrorCode = serde_json::from_str(r#""rate_limited""#).unwrap();
/// assert_eq!(future, ErrorCode::Unknown("rate_limited".to_owned()));
/// assert!(!future.is_known());
/// assert_eq!(serde_json::to_string(&future).unwrap(), r#""rate_limited""#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorCode {
    /// Request body failed validation (HTTP 400).
    ValidationFailed,
    /// Login identifier/password mismatch (HTTP 401).
    InvalidCredentials,
    /// Missing, malformed, or expired token (HTTP 401).
    Unauthenticated,
    /// Resource absent or not owned by the caller (HTTP 404).
    NotFound,
    /// Registration username already exists (HTTP 409).
    UsernameTaken,
    /// Registration email already exists (HTTP 409).
    EmailTaken,
    /// Profile name already used by the account on create/rename (HTTP 409).
    ProfileNameTaken,
    /// Refused delete of the account's only remaining profile (HTTP 409).
    LastProfile,
    /// Unexpected server error (HTTP 500); the message is generic.
    Internal,
    /// A code not known to this version of the crate. Carries the raw wire string so it
    /// round-trips losslessly. Treat as a generic, unmatched error.
    Unknown(String),
}

impl ErrorCode {
    /// The wire string for this code.
    ///
    /// # Examples
    ///
    /// ```
    /// use contract::ErrorCode;
    ///
    /// assert_eq!(ErrorCode::UsernameTaken.as_str(), "username_taken");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::ValidationFailed => "validation_failed",
            Self::InvalidCredentials => "invalid_credentials",
            Self::Unauthenticated => "unauthenticated",
            Self::NotFound => "not_found",
            Self::UsernameTaken => "username_taken",
            Self::EmailTaken => "email_taken",
            Self::ProfileNameTaken => "profile_name_taken",
            Self::LastProfile => "last_profile",
            Self::Internal => "internal",
            Self::Unknown(raw) => raw,
        }
    }

    /// Whether this code is one this crate version recognizes (i.e. not
    /// [`ErrorCode::Unknown`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use contract::ErrorCode;
    ///
    /// assert!(ErrorCode::NotFound.is_known());
    /// assert!(!ErrorCode::Unknown("rate_limited".to_owned()).is_known());
    /// ```
    #[must_use]
    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown(_))
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for ErrorCode {
    fn from(value: &str) -> Self {
        match value {
            "validation_failed" => Self::ValidationFailed,
            "invalid_credentials" => Self::InvalidCredentials,
            "unauthenticated" => Self::Unauthenticated,
            "not_found" => Self::NotFound,
            "username_taken" => Self::UsernameTaken,
            "email_taken" => Self::EmailTaken,
            "profile_name_taken" => Self::ProfileNameTaken,
            "last_profile" => Self::LastProfile,
            "internal" => Self::Internal,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

impl Serialize for ErrorCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ErrorCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::from(raw.as_str()))
    }
}

/// The standard error response body: an optional [`ErrorCode`] plus a human-readable message.
///
/// Every error response is the standard HTTP status code **plus** this JSON body. `code`
/// lets the TUI branch on specific cases; `message` is always present and human-readable.
/// `code` is omitted from the JSON when absent.
///
/// # Examples
///
/// ```
/// use contract::{ErrorBody, ErrorCode};
///
/// let body = ErrorBody {
///     code: Some(ErrorCode::NotFound),
///     message: "no such task".to_owned(),
/// };
/// let json = serde_json::to_value(&body).unwrap();
/// assert_eq!(json["code"], "not_found");
/// assert_eq!(json["message"], "no such task");
///
/// // `code` is optional and omitted when `None`.
/// let bare = ErrorBody { code: None, message: "something went wrong".to_owned() };
/// assert_eq!(serde_json::to_string(&bare).unwrap(), r#"{"message":"something went wrong"}"#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Optional machine-matchable code; omitted from JSON when `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<ErrorCode>,
    /// Human-readable error message; always present.
    pub message: String,
}
