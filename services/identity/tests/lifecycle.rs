use identity::crypto::jwt::{Audience, verify_jwt};
use identity::crypto::kek::Kek;
use identity::crypto::{
    Algorithm, Claims, InMemoryKeyStore, KeyStore, VerificationOptions, create_signing_key,
    get_jwks_json, rotate_signing_key, sign_claims,
};

fn test_kek() -> Kek {
    Kek::from_bytes(vec![0xAA; 32]).unwrap()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[tokio::test]
async fn full_key_lifecycle() {
    let store = InMemoryKeyStore::new();
    let kek = test_kek();

    // 1. Create a signing key
    let kid = create_signing_key(Algorithm::ES256, &kek, &store)
        .await
        .unwrap();

    // 2. Sign a JWT
    let claims = Claims {
        iss: Some("https://home.ryanseipp.com".into()),
        sub: Some("user-42".into()),
        aud: Some(Audience::Single("https://api.home.ryanseipp.com".into())),
        exp: Some(now_secs() + 3600),
        iat: Some(now_secs()),
        name: Some("Ryan Seipp".into()),
        given_name: Some("Ryan".into()),
        family_name: Some("Seipp".into()),
        email: Some("ryan@example.com".into()),
        email_verified: Some(true),
        preferred_username: Some("ryanseipp".into()),
        ..Default::default()
    };

    let jwt = sign_claims(&claims, &kek, &store).await.unwrap();
    assert_eq!(jwt.kid, kid);
    assert_eq!(jwt.token.split('.').count(), 3);

    // 3. Retrieve JWKS
    let jwks_json = get_jwks_json(&store).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&jwks_json).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 1);

    // 4. Verify the JWT using the JWKS public key
    let public_keys = store.list_public_keys().await.unwrap();
    let public_jwk = public_keys.iter().find(|k| k.kid == kid).unwrap();

    let options = VerificationOptions {
        allowed_algorithms: &[Algorithm::ES256],
        issuer: Some("https://home.ryanseipp.com"),
        audience: Some("https://api.home.ryanseipp.com"),
        current_time: now_secs(),
        leeway: 0,
    };

    let verified = verify_jwt(&jwt.token, public_jwk, &options).unwrap();
    assert_eq!(verified.sub.as_deref(), Some("user-42"));
    assert_eq!(verified.email.as_deref(), Some("ryan@example.com"));
    assert_eq!(verified.name.as_deref(), Some("Ryan Seipp"));
}

#[tokio::test]
async fn full_key_lifecycle_es384() {
    let store = InMemoryKeyStore::new();
    let kek = test_kek();

    let kid = create_signing_key(Algorithm::ES384, &kek, &store)
        .await
        .unwrap();

    let claims = Claims {
        sub: Some("user-es384".into()),
        exp: Some(now_secs() + 3600),
        iat: Some(now_secs()),
        ..Default::default()
    };

    let jwt = sign_claims(&claims, &kek, &store).await.unwrap();
    assert_eq!(jwt.kid, kid);

    let public_keys = store.list_public_keys().await.unwrap();
    let public_jwk = public_keys.iter().find(|k| k.kid == kid).unwrap();

    let options = VerificationOptions {
        allowed_algorithms: &[Algorithm::ES384],
        issuer: None,
        audience: None,
        current_time: now_secs(),
        leeway: 0,
    };

    let verified = verify_jwt(&jwt.token, public_jwk, &options).unwrap();
    assert_eq!(verified.sub.as_deref(), Some("user-es384"));
}

#[tokio::test]
async fn key_rotation_lifecycle() {
    let store = InMemoryKeyStore::new();
    let kek = test_kek();

    // Create initial key
    let kid1 = create_signing_key(Algorithm::ES256, &kek, &store)
        .await
        .unwrap();

    // Sign with key 1
    let claims = Claims {
        sub: Some("user-1".into()),
        exp: Some(now_secs() + 3600),
        iat: Some(now_secs()),
        ..Default::default()
    };
    let jwt1 = sign_claims(&claims, &kek, &store).await.unwrap();
    assert_eq!(jwt1.kid, kid1);

    // Rotate: key 1 becomes "rotated", key 2 becomes "active"
    let kid2 = rotate_signing_key(Algorithm::ES256, &kek, &store)
        .await
        .unwrap();
    assert_ne!(kid1, kid2);

    // Sign with key 2 (now active)
    let jwt2 = sign_claims(&claims, &kek, &store).await.unwrap();
    assert_eq!(jwt2.kid, kid2);

    // JWKS should contain both keys (key1 is rotated, not revoked)
    let jwks_json = get_jwks_json(&store).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&jwks_json).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 2);

    // Both JWTs should verify with their respective public keys
    let public_keys = store.list_public_keys().await.unwrap();
    let options = VerificationOptions {
        allowed_algorithms: &[Algorithm::ES256],
        issuer: None,
        audience: None,
        current_time: now_secs(),
        leeway: 0,
    };

    let pk1 = public_keys.iter().find(|k| k.kid == kid1).unwrap();
    let pk2 = public_keys.iter().find(|k| k.kid == kid2).unwrap();

    assert!(verify_jwt(&jwt1.token, pk1, &options).is_ok());
    assert!(verify_jwt(&jwt2.token, pk2, &options).is_ok());

    // Cross-verification fails (jwt1 signed by key1, verified against key2)
    assert!(verify_jwt(&jwt1.token, pk2, &options).is_err());
    assert!(verify_jwt(&jwt2.token, pk1, &options).is_err());
}

#[tokio::test]
async fn revoked_key_excluded_from_jwks() {
    let store = InMemoryKeyStore::new();
    let kek = test_kek();

    let kid = create_signing_key(Algorithm::ES256, &kek, &store)
        .await
        .unwrap();

    // JWKS has 1 key
    let jwks_json = get_jwks_json(&store).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&jwks_json).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 1);

    // Revoke it
    store.revoke_key(&kid).await.unwrap();

    // JWKS is now empty
    let jwks_json = get_jwks_json(&store).await.unwrap();
    let jwks: serde_json::Value = serde_json::from_str(&jwks_json).unwrap();
    assert_eq!(jwks["keys"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn sign_without_active_key_fails() {
    let store = InMemoryKeyStore::new();
    let kek = test_kek();

    let claims = Claims {
        sub: Some("user".into()),
        ..Default::default()
    };

    let result = sign_claims(&claims, &kek, &store).await;
    assert!(result.is_err());
}
