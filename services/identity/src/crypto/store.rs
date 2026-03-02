use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::crypto::CryptoError;
use crate::crypto::jwk::{Algorithm, Jwk};

/// Status of a key in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStatus {
    /// Key is the active signing key.
    Active,
    /// Key is valid for verification but not used for new signatures.
    Rotated,
    /// Key has been revoked and should not be used.
    Revoked,
}

/// A stored key entry.
#[derive(Debug, Clone)]
pub struct KeyEntry {
    /// Unique key identifier (UUID v4).
    pub kid: String,
    /// Signing algorithm for this key.
    pub algorithm: Algorithm,
    /// Encrypted private key material (nonce || ciphertext || tag).
    pub encrypted_private_key: Vec<u8>,
    /// Public JWK (for JWKS endpoint).
    pub public_jwk: Jwk,
    /// Current status of this key.
    pub status: KeyStatus,
    /// When the key was created (seconds since epoch).
    pub created_at: u64,
}

/// Async key storage trait.
///
/// Implementations may back this with PostgreSQL (via sqlx), a KMS,
/// or the provided in-memory store for testing.
#[async_trait]
pub trait KeyStore: Send + Sync {
    /// Store a new key entry.
    async fn store_key(&self, entry: &KeyEntry) -> Result<(), CryptoError>;

    /// Retrieve a key entry by its kid.
    async fn get_key(&self, kid: &str) -> Result<Option<KeyEntry>, CryptoError>;

    /// Get the currently active signing key.
    /// Returns `None` if no active key exists.
    async fn get_active_signing_key(&self) -> Result<Option<KeyEntry>, CryptoError>;

    /// List all public JWKs for non-revoked keys (Active + Rotated).
    /// These are the keys published at the JWKS endpoint.
    async fn list_public_keys(&self) -> Result<Vec<Jwk>, CryptoError>;

    /// Mark a key as rotated (no longer used for signing, still valid for verification).
    async fn rotate_key(&self, kid: &str) -> Result<(), CryptoError>;

    /// Mark a key as revoked (should not be used at all).
    async fn revoke_key(&self, kid: &str) -> Result<(), CryptoError>;
}

/// In-memory key store for testing and development.
#[derive(Debug, Clone, Default)]
pub struct InMemoryKeyStore {
    keys: Arc<RwLock<HashMap<String, KeyEntry>>>,
}

impl InMemoryKeyStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl KeyStore for InMemoryKeyStore {
    async fn store_key(&self, entry: &KeyEntry) -> Result<(), CryptoError> {
        let mut keys = self.keys.write().await;
        keys.insert(entry.kid.clone(), entry.clone());
        Ok(())
    }

    async fn get_key(&self, kid: &str) -> Result<Option<KeyEntry>, CryptoError> {
        let keys = self.keys.read().await;
        Ok(keys.get(kid).cloned())
    }

    async fn get_active_signing_key(&self) -> Result<Option<KeyEntry>, CryptoError> {
        let keys = self.keys.read().await;
        Ok(keys
            .values()
            .find(|k| k.status == KeyStatus::Active)
            .cloned())
    }

    async fn list_public_keys(&self) -> Result<Vec<Jwk>, CryptoError> {
        let keys = self.keys.read().await;
        Ok(keys
            .values()
            .filter(|k| k.status != KeyStatus::Revoked)
            .map(|k| k.public_jwk.clone())
            .collect())
    }

    async fn rotate_key(&self, kid: &str) -> Result<(), CryptoError> {
        let mut keys = self.keys.write().await;
        match keys.get_mut(kid) {
            Some(entry) => {
                entry.status = KeyStatus::Rotated;
                Ok(())
            }
            None => Err(CryptoError::KeyNotFound),
        }
    }

    async fn revoke_key(&self, kid: &str) -> Result<(), CryptoError> {
        let mut keys = self.keys.write().await;
        match keys.get_mut(kid) {
            Some(entry) => {
                entry.status = KeyStatus::Revoked;
                Ok(())
            }
            None => Err(CryptoError::KeyNotFound),
        }
    }
}

// ============================================================================
// Future PostgreSQL implementation (commented out until sqlx is added)
// ============================================================================
//
// Expected table schema:
//
// CREATE TABLE signing_keys (
//     kid TEXT PRIMARY KEY,
//     algorithm TEXT NOT NULL,
//     encrypted_private_key BYTEA NOT NULL,
//     public_jwk JSONB NOT NULL,
//     status TEXT NOT NULL DEFAULT 'active',
//     created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
// );
//
// CREATE INDEX idx_signing_keys_status ON signing_keys (status);
//
// pub struct PgKeyStore {
//     pool: sqlx::PgPool,
// }
//
// #[async_trait]
// impl KeyStore for PgKeyStore {
//     async fn store_key(&self, entry: &KeyEntry) -> Result<(), CryptoError> {
//         sqlx::query!(
//             r#"INSERT INTO signing_keys (kid, algorithm, encrypted_private_key, public_jwk, status, created_at)
//                VALUES ($1, $2, $3, $4, $5, to_timestamp($6))"#,
//             entry.kid,
//             entry.algorithm.as_str(),
//             &entry.encrypted_private_key,
//             serde_json::to_value(&entry.public_jwk).map_err(|_| CryptoError::Serialization)?,
//             match entry.status {
//                 KeyStatus::Active => "active",
//                 KeyStatus::Rotated => "rotated",
//                 KeyStatus::Revoked => "revoked",
//             },
//             entry.created_at as f64,
//         )
//         .execute(&self.pool)
//         .await
//         .map_err(|e| CryptoError::Store(Box::new(e)))?;
//         Ok(())
//     }
//
//     async fn get_key(&self, kid: &str) -> Result<Option<KeyEntry>, CryptoError> {
//         // sqlx::query_as!(... WHERE kid = $1 ...)
//         todo!()
//     }
//
//     async fn get_active_signing_key(&self) -> Result<Option<KeyEntry>, CryptoError> {
//         // sqlx::query_as!(
//         //     KeyEntryRow,
//         //     r#"SELECT kid, algorithm, encrypted_private_key, public_jwk, status, created_at
//         //        FROM signing_keys
//         //        WHERE status = 'active'
//         //        ORDER BY created_at DESC
//         //        LIMIT 1"#,
//         // )
//         // .fetch_optional(&self.pool)
//         // .await
//         // .map_err(|e| CryptoError::Store(Box::new(e)))?
//         // .map(|row| row.try_into())
//         // .transpose()
//         todo!()
//     }
//
//     async fn list_public_keys(&self) -> Result<Vec<Jwk>, CryptoError> {
//         // sqlx::query_as!(... WHERE status != 'revoked' ...)
//         todo!()
//     }
//
//     async fn rotate_key(&self, kid: &str) -> Result<(), CryptoError> {
//         // sqlx::query!(... UPDATE signing_keys SET status = 'rotated' WHERE kid = $1 ...)
//         todo!()
//     }
//
//     async fn revoke_key(&self, kid: &str) -> Result<(), CryptoError> {
//         // sqlx::query!(... UPDATE signing_keys SET status = 'revoked' WHERE kid = $1 ...)
//         todo!()
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::jwk::{Algorithm, generate_key_pair};

    fn sample_entry(status: KeyStatus) -> KeyEntry {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        KeyEntry {
            kid: kp.kid,
            algorithm: Algorithm::ES256,
            encrypted_private_key: vec![0xDE; 64],
            public_jwk: kp.public_jwk,
            status,
            created_at: 1700000000,
        }
    }

    #[tokio::test]
    async fn store_and_retrieve_key() {
        let store = InMemoryKeyStore::new();
        let entry = sample_entry(KeyStatus::Active);
        let kid = entry.kid.clone();

        store.store_key(&entry).await.unwrap();

        let retrieved = store.get_key(&kid).await.unwrap().unwrap();
        assert_eq!(retrieved.kid, kid);
        assert_eq!(retrieved.status, KeyStatus::Active);
    }

    #[tokio::test]
    async fn get_active_signing_key() {
        let store = InMemoryKeyStore::new();

        let rotated = sample_entry(KeyStatus::Rotated);
        let active = sample_entry(KeyStatus::Active);
        let active_kid = active.kid.clone();

        store.store_key(&rotated).await.unwrap();
        store.store_key(&active).await.unwrap();

        let result = store.get_active_signing_key().await.unwrap().unwrap();
        assert_eq!(result.kid, active_kid);
    }

    #[tokio::test]
    async fn list_public_keys_excludes_revoked() {
        let store = InMemoryKeyStore::new();

        let active = sample_entry(KeyStatus::Active);
        let rotated = sample_entry(KeyStatus::Rotated);
        let revoked = sample_entry(KeyStatus::Revoked);

        store.store_key(&active).await.unwrap();
        store.store_key(&rotated).await.unwrap();
        store.store_key(&revoked).await.unwrap();

        let public_keys = store.list_public_keys().await.unwrap();
        assert_eq!(public_keys.len(), 2);
    }

    #[tokio::test]
    async fn rotate_key() {
        let store = InMemoryKeyStore::new();
        let entry = sample_entry(KeyStatus::Active);
        let kid = entry.kid.clone();

        store.store_key(&entry).await.unwrap();
        store.rotate_key(&kid).await.unwrap();

        let retrieved = store.get_key(&kid).await.unwrap().unwrap();
        assert_eq!(retrieved.status, KeyStatus::Rotated);
    }

    #[tokio::test]
    async fn revoke_key() {
        let store = InMemoryKeyStore::new();
        let entry = sample_entry(KeyStatus::Active);
        let kid = entry.kid.clone();

        store.store_key(&entry).await.unwrap();
        store.revoke_key(&kid).await.unwrap();

        let retrieved = store.get_key(&kid).await.unwrap().unwrap();
        assert_eq!(retrieved.status, KeyStatus::Revoked);
    }

    #[tokio::test]
    async fn rotate_nonexistent_key_fails() {
        let store = InMemoryKeyStore::new();
        let result = store.rotate_key("nonexistent").await;
        assert!(matches!(result, Err(CryptoError::KeyNotFound)));
    }

    #[tokio::test]
    async fn get_nonexistent_key_returns_none() {
        let store = InMemoryKeyStore::new();
        let result = store.get_key("nonexistent").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn no_active_key_returns_none() {
        let store = InMemoryKeyStore::new();
        let result = store.get_active_signing_key().await.unwrap();
        assert!(result.is_none());
    }
}
