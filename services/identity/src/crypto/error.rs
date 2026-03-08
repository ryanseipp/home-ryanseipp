use thiserror::Error;

/// Unified error type for all cryptographic operations.
///
/// Error messages are intentionally generic to avoid leaking information
/// about internal cryptographic state (per OWASP Key Management guidance).
#[derive(Debug, PartialEq, Eq, Error)]
pub enum CryptoError {
    #[error("key generation failed")]
    KeyGeneration,

    #[error("signing operation failed")]
    Signing,

    #[error("verification failed")]
    Verification,

    #[error("encryption failed")]
    Encryption,

    #[error("decryption failed")]
    Decryption,

    #[error("invalid key material")]
    InvalidKeyMaterial,

    #[error("invalid key encryption key")]
    InvalidKek,

    #[error("key not found")]
    KeyNotFound,

    #[error("serialization failed")]
    Serialization,

    #[error("invalid token")]
    InvalidToken,

    #[error("token expired")]
    TokenExpired,

    #[error("algorithm mismatch")]
    AlgorithmMismatch,

    #[error("kek loading failed: {0}")]
    KekLoad(String),

    #[error("random number generation failed")]
    RngFailure,
}
