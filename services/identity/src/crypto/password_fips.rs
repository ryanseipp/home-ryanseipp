#![cfg(feature = "fips")]

use aws_lc_rs::pbkdf2;
use password_hash::{
    Ident, Output, ParamsString, PasswordHash, PasswordHasher, PasswordVerifier, Salt, SaltString,
};

/// PBKDF2-HMAC-SHA-256 iterations per OWASP 2023 recommendation.
const ITERATIONS: u32 = 600_000;

/// PHC algorithm identifier for PBKDF2-SHA-256.
const ALG_ID: Ident<'_> = Ident::new_unwrap("pbkdf2-sha256");

/// PBKDF2-HMAC-SHA-256 password hasher backed by aws-lc-rs.
///
/// Implements the `password-hash` crate traits for PHC string format support.
/// Used only in FIPS builds where Argon2 is not available.
pub struct AwsLcPbkdf2;

impl PasswordHasher for AwsLcPbkdf2 {
    type Params = ();

    fn hash_password_customized<'a>(
        &self,
        password: &[u8],
        _algorithm: Option<Ident<'a>>,
        _version: Option<u32>,
        _params: (),
        salt: impl Into<Salt<'a>>,
    ) -> password_hash::Result<PasswordHash<'a>> {
        let salt: Salt<'a> = salt.into();

        let mut output_bytes = [0u8; 32]; // SHA-256 output length
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(ITERATIONS).unwrap(),
            salt.as_str().as_bytes(),
            password,
            &mut output_bytes,
        );

        let output = Output::new(&output_bytes).map_err(|_| password_hash::Error::OutputSize)?;

        let mut params = ParamsString::new();
        params.add_decimal("i", ITERATIONS).map_err(|_| {
            password_hash::Error::ParamValueInvalid(password_hash::errors::InvalidValue::Malformed)
        })?;

        Ok(PasswordHash {
            algorithm: ALG_ID,
            version: None,
            params,
            salt: Some(salt),
            hash: Some(output),
        })
    }
}

impl PasswordVerifier for AwsLcPbkdf2 {
    fn verify_password(
        &self,
        password: &[u8],
        hash: &PasswordHash<'_>,
    ) -> password_hash::Result<()> {
        let iterations: u32 = hash
            .params
            .iter()
            .find(|(key, _)| key == &Ident::new_unwrap("i"))
            .and_then(|(_, val)| val.decimal().ok())
            .ok_or(password_hash::Error::ParamValueInvalid(
                password_hash::errors::InvalidValue::Malformed,
            ))?;

        let salt = hash.salt.ok_or(password_hash::Error::SaltInvalid(
            password_hash::errors::InvalidValue::Malformed,
        ))?;

        let expected_output = hash.hash.ok_or(password_hash::Error::OutputSize)?;

        let mut computed = vec![0u8; expected_output.len()];
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(iterations).ok_or(
                password_hash::Error::ParamValueInvalid(
                    password_hash::errors::InvalidValue::Malformed,
                ),
            )?,
            salt.as_str().as_bytes(),
            password,
            &mut computed,
        );

        // Use pbkdf2::verify for constant-time comparison
        pbkdf2::verify(
            pbkdf2::PBKDF2_HMAC_SHA256,
            std::num::NonZeroU32::new(iterations).unwrap(),
            salt.as_str().as_bytes(),
            password,
            expected_output.as_bytes(),
        )
        .map_err(|_| password_hash::Error::Password)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phc_roundtrip() {
        let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
        let hash = AwsLcPbkdf2.hash_password(b"test-password", &salt).unwrap();
        let phc_string = hash.to_string();

        // Parse back and verify
        let parsed = PasswordHash::new(&phc_string).unwrap();
        assert_eq!(parsed.algorithm, ALG_ID);

        AwsLcPbkdf2
            .verify_password(b"test-password", &parsed)
            .unwrap();
    }

    #[test]
    fn correct_iteration_count_in_output() {
        let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
        let hash = AwsLcPbkdf2.hash_password(b"password", &salt).unwrap();

        let iterations: u32 = hash
            .params
            .iter()
            .find(|(key, _)| key == &Ident::new_unwrap("i"))
            .and_then(|(_, val)| val.decimal().ok())
            .unwrap();

        assert_eq!(iterations, ITERATIONS);
    }

    #[test]
    fn wrong_password_fails_verification() {
        let salt = SaltString::generate(&mut password_hash::rand_core::OsRng);
        let hash = AwsLcPbkdf2
            .hash_password(b"correct-password", &salt)
            .unwrap();
        let phc_string = hash.to_string();
        let parsed = PasswordHash::new(&phc_string).unwrap();

        let result = AwsLcPbkdf2.verify_password(b"wrong-password", &parsed);
        assert!(result.is_err());
    }
}
