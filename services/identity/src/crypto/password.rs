#[cfg(all(feature = "argon2", feature = "fips"))]
compile_error!(
    "features `argon2` and `fips` are mutually exclusive; \
     use `--no-default-features --features fips` for FIPS builds"
);

#[cfg(not(any(feature = "argon2", feature = "fips")))]
compile_error!("either `argon2` or `fips` feature must be enabled");

use password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("password hashing failed")]
    HashingFailed,

    #[error("password verification failed")]
    VerificationFailed,

    #[error("invalid password hash format")]
    InvalidFormat,
}

/// Hash a password using the configured algorithm.
///
/// - Default: Argon2id (m=19456 KiB, t=2, p=1) per OWASP 2023.
/// - FIPS: PBKDF2-HMAC-SHA-256 (600,000 iterations) per NIST SP 800-132.
///
/// Returns a PHC-format string that embeds algorithm, parameters, salt, and hash.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);

    #[cfg(feature = "argon2")]
    {
        use argon2::{Algorithm, Argon2, Params, Version};

        let params = Params::new(19456, 2, 1, None).map_err(|_| PasswordError::HashingFailed)?;
        let hasher = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let hash = hasher
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| PasswordError::HashingFailed)?;

        Ok(hash.to_string())
    }

    #[cfg(feature = "fips")]
    {
        let hasher = super::password_fips::AwsLcPbkdf2;

        let hash = hasher
            .hash_password(password.as_bytes(), &salt)
            .map_err(|_| PasswordError::HashingFailed)?;

        Ok(hash.to_string())
    }
}

/// Verify a password against a PHC-format hash string.
///
/// Automatically detects the algorithm from the hash string (Argon2id or PBKDF2).
pub fn verify_password(password: &str, phc_hash: &str) -> Result<(), PasswordError> {
    let parsed = PasswordHash::new(phc_hash).map_err(|_| PasswordError::InvalidFormat)?;

    #[cfg(feature = "argon2")]
    {
        use argon2::Argon2;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| PasswordError::VerificationFailed)
    }

    #[cfg(feature = "fips")]
    {
        super::password_fips::AwsLcPbkdf2
            .verify_password(password.as_bytes(), &parsed)
            .map_err(|_| PasswordError::VerificationFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_produces_valid_phc_string() {
        let hash = hash_password("test-password-123").unwrap();
        // PHC strings start with $
        assert!(hash.starts_with('$'));
        // Should be parseable
        let parsed = PasswordHash::new(&hash).unwrap();
        assert!(parsed.salt.is_some());
    }

    #[test]
    fn hash_is_unique_per_call() {
        let h1 = hash_password("same-password").unwrap();
        let h2 = hash_password("same-password").unwrap();
        assert_ne!(h1, h2); // Different salts
    }

    #[test]
    fn roundtrip_hash_verify() {
        let password = "correct-horse-battery-staple";
        let hash = hash_password(password).unwrap();
        verify_password(password, &hash).unwrap();
    }

    #[test]
    fn wrong_password_fails() {
        let hash = hash_password("real-password").unwrap();
        let result = verify_password("wrong-password", &hash);
        assert!(matches!(result, Err(PasswordError::VerificationFailed)));
    }

    #[test]
    fn invalid_hash_format_fails() {
        let result = verify_password("password", "not-a-phc-string");
        assert!(matches!(result, Err(PasswordError::InvalidFormat)));
    }
}
