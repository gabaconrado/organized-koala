//! Authentication: argon2 password hashing, JWT HS256 session issue/verify, and the axum
//! extractor that turns a `Bearer` token into the authenticated user id for handlers.

mod jwt;
mod password;
mod session;

pub use jwt::Jwt;
pub use password::{hash_password, verify_password};
pub use session::AuthUser;
