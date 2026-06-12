//! Argon2 password hashing. Plaintext is exposed from the [`Password`] secret only at the
//! point of hashing/verification and never stored or logged. Hashes are PHC strings.

use anyhow::Context as _;
use argon2::Argon2;
use contract::Password;
use password_hash::rand_core::OsRng;
use password_hash::{PasswordHash, PasswordHasher as _, PasswordVerifier as _, SaltString};

/// Hash a plaintext password into a PHC string suitable for storage.
pub fn hash_password(password: &Password) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(password.expose().as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!("hashing password: {error}"))?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against a stored PHC hash. Returns `Ok(true)` on a match,
/// `Ok(false)` on a mismatch, and an error only if the stored hash is malformed.
pub fn verify_password(password: &Password, phc: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(phc)
        .map_err(|error| anyhow::anyhow!("parsing stored hash: {error}"))
        .context("stored password hash is malformed")?;
    match Argon2::default().verify_password(password.expose().as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(password_hash::Error::Password) => Ok(false),
        Err(error) => Err(anyhow::anyhow!("verifying password: {error}")),
    }
}
