pub mod encryption;
pub mod error;
pub mod jwk;
pub mod jwt;
pub mod kek;
pub mod store;

pub use error::CryptoError;
pub use jwk::{Algorithm, GeneratedKeyPair, Jwk, JwkSet};
pub use jwt::{Claims, EncodedJwt, VerificationOptions};
pub use kek::Kek;
pub use store::{InMemoryKeyStore, KeyEntry, KeyStatus, KeyStore};

use encryption::{EncryptedKey, decrypt_private_key, encrypt_private_key};
use jwk::generate_key_pair;
use jwt::{key_pair_from_pkcs8, sign_jwt};

/// Generate a new signing key, encrypt the private key, and store it.
///
/// This is the primary key creation workflow:
/// 1. Generate ECDSA key pair
/// 2. Encrypt private key with the KEK (AAD = kid for binding)
/// 3. Store the encrypted key + public JWK
/// 4. Return the kid
pub async fn create_signing_key(
    algorithm: Algorithm,
    kek: &Kek,
    store: &dyn KeyStore,
) -> Result<String, CryptoError> {
    let generated = generate_key_pair(algorithm)?;

    let encrypted = encrypt_private_key(
        kek.as_bytes(),
        generated.private_key_pkcs8.as_bytes(),
        generated.kid.as_bytes(),
    )?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let entry = KeyEntry {
        kid: generated.kid.clone(),
        algorithm,
        encrypted_private_key: encrypted.as_bytes().to_vec(),
        public_jwk: generated.public_jwk,
        status: KeyStatus::Active,
        created_at: now,
    };

    store.store_key(&entry).await?;

    Ok(generated.kid)
}

/// Sign a JWT using the active signing key from the store.
///
/// 1. Retrieve the active signing key
/// 2. Decrypt the private key
/// 3. Reconstruct the key pair
/// 4. Sign the JWT
pub async fn sign_claims(
    claims: &Claims,
    kek: &Kek,
    store: &dyn KeyStore,
) -> Result<EncodedJwt, CryptoError> {
    let entry = store
        .get_active_signing_key()
        .await?
        .ok_or(CryptoError::KeyNotFound)?;

    let encrypted = EncryptedKey::from_bytes(entry.encrypted_private_key.clone())?;
    let pkcs8_der = decrypt_private_key(kek.as_bytes(), &encrypted, entry.kid.as_bytes())?;

    let key_pair = key_pair_from_pkcs8(entry.algorithm, &pkcs8_der)?;

    sign_jwt(entry.algorithm, &entry.kid, claims, &key_pair)
}

/// Get the JWKS (all non-revoked public keys) as a JSON string.
///
/// This is what the GetJwks gRPC endpoint will return.
pub async fn get_jwks_json(store: &dyn KeyStore) -> Result<String, CryptoError> {
    let public_keys = store.list_public_keys().await?;
    let jwk_set = jwk::build_jwk_set(public_keys);
    serde_json::to_string(&jwk_set).map_err(|_| CryptoError::Serialization)
}

/// Rotate the active key: mark it as rotated and create a new active key.
///
/// The rotated key remains in the JWKS for verification of existing tokens.
pub async fn rotate_signing_key(
    algorithm: Algorithm,
    kek: &Kek,
    store: &dyn KeyStore,
) -> Result<String, CryptoError> {
    if let Some(current) = store.get_active_signing_key().await? {
        store.rotate_key(&current.kid).await?;
    }

    create_signing_key(algorithm, kek, store).await
}
