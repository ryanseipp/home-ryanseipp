use proptest::collection;
use proptest::prelude::*;

use identity::crypto::Jwk;
use identity::crypto::encryption::{decrypt_private_key, encrypt_private_key};
use identity::crypto::jwk::{Algorithm, generate_key_pair};
use identity::crypto::jwt::{
    Audience, Claims, VerificationOptions, key_pair_from_pkcs8, sign_jwt, verify_jwt,
};
use jsonwebtoken::Algorithm as JtAlgorithm;
use jsonwebtoken::errors::Error as JtError;
use jsonwebtoken::jwk::{
    AlgorithmParameters, CommonParameters, EllipticCurve, EllipticCurveKeyParameters,
    EllipticCurveKeyType, Jwk as JtJwk, PublicKeyUse,
};
use jsonwebtoken::{DecodingKey, Validation, decode};

/// Verify a JWT produced by our implementation using the `jsonwebtoken` crate
/// (`RustCrypto` backend) — a completely independent crypto stack.
///
/// This confirms our JWTs and JWKs are standards-compliant and interoperable.
fn verify_with_jsonwebtoken(
    token: &str,
    jwk: &Jwk,
    expected_alg: JtAlgorithm,
) -> Result<serde_json::Value, JtError> {
    // Build a jsonwebtoken::Jwk from our JWK fields
    let jt_jwk = JtJwk {
        common: CommonParameters {
            public_key_use: Some(PublicKeyUse::Signature),
            key_id: Some(jwk.kid.clone()),
            key_algorithm: None,
            ..Default::default()
        },
        algorithm: AlgorithmParameters::EllipticCurve(EllipticCurveKeyParameters {
            key_type: EllipticCurveKeyType::EC,
            curve: match jwk.crv.as_str() {
                "P-256" => EllipticCurve::P256,
                "P-384" => EllipticCurve::P384,
                _ => unreachable!(),
            },
            x: jwk.x.clone(),
            y: jwk.y.clone(),
        }),
    };

    let decoding_key = DecodingKey::from_jwk(&jt_jwk)?;

    let mut validation = Validation::new(expected_alg);
    validation.validate_aud = false; // we validate separately
    validation.required_spec_claims.clear();

    decode::<serde_json::Value>(token, &decoding_key, &validation).map(|data| data.claims)
}

fn arbitrary_kek() -> impl Strategy<Value = Vec<u8>> {
    collection::vec(any::<u8>(), 32..=32)
}

fn arbitrary_plaintext() -> impl Strategy<Value = Vec<u8>> {
    collection::vec(any::<u8>(), 1..1024)
}

proptest! {
    #[test]
    fn encrypt_decrypt_roundtrip_property(
        kek in arbitrary_kek(),
        plaintext in arbitrary_plaintext(),
        aad in "[a-z0-9]{0,64}",
    ) {
        let encrypted = encrypt_private_key(&kek, &plaintext, aad.as_bytes()).unwrap();
        let decrypted = decrypt_private_key(&kek, &encrypted, aad.as_bytes()).unwrap();
        prop_assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn sign_verify_roundtrip_es256(
        sub in "[a-z0-9-]{1,64}",
        iss in "https://[a-z.]{3,30}",
    ) {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair = key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            iss: Some(iss.clone()),
            sub: Some(sub.clone()),
            aud: Some(Audience::Single("test-aud".into())),
            exp: Some(now + 3600),
            iat: Some(now),
            ..Default::default()
        };

        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: Some(&iss),
            audience: Some("test-aud"),
            current_time: now,
            leeway: 0,
        };

        let verified = verify_jwt(&encoded.token, &kp.public_jwk, &options).unwrap();
        prop_assert_eq!(verified.sub.as_deref(), Some(sub.as_str()));
        prop_assert_eq!(verified.iss.as_deref(), Some(iss.as_str()));
    }

    #[test]
    fn sign_verify_roundtrip_es384(
        sub in "[a-z0-9-]{1,64}",
    ) {
        let kp = generate_key_pair(Algorithm::ES384).unwrap();
        let key_pair = key_pair_from_pkcs8(Algorithm::ES384, kp.private_key_pkcs8.as_bytes()).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: Some(sub.clone()),
            exp: Some(now + 3600),
            iat: Some(now),
            ..Default::default()
        };

        let encoded = sign_jwt(Algorithm::ES384, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES384],
            issuer: None,
            audience: None,
            current_time: now,
            leeway: 0,
        };

        let verified = verify_jwt(&encoded.token, &kp.public_jwk, &options).unwrap();
        prop_assert_eq!(verified.sub.as_deref(), Some(sub.as_str()));
    }

    #[test]
    fn generated_kids_are_unique(_ in 0..100u32) {
        let kp1 = generate_key_pair(Algorithm::ES256).unwrap();
        let kp2 = generate_key_pair(Algorithm::ES256).unwrap();
        prop_assert_ne!(kp1.kid, kp2.kid);
    }

    /// Any JWT + JWK we produce for ES256 can be verified by jsonwebtoken (RustCrypto backend).
    #[test]
    fn interop_jsonwebtoken_verifies_our_es256(
        sub in "[a-z0-9-]{1,64}",
        iss in "https://[a-z.]{3,30}",
    ) {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair = key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            iss: Some(iss.clone()),
            sub: Some(sub.clone()),
            aud: Some(Audience::Single("test-aud".into())),
            exp: Some(now + 3600),
            iat: Some(now),
            ..Default::default()
        };

        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        // Verify with jsonwebtoken (independent RustCrypto backend)
        let verified = verify_with_jsonwebtoken(
            &encoded.token,
            &kp.public_jwk,
            JtAlgorithm::ES256,
        ).unwrap();

        prop_assert_eq!(verified["sub"].as_str(), Some(sub.as_str()));
        prop_assert_eq!(verified["iss"].as_str(), Some(iss.as_str()));
    }

    /// Any JWT + JWK we produce for ES384 can be verified by jsonwebtoken (RustCrypto backend).
    #[test]
    fn interop_jsonwebtoken_verifies_our_es384(
        sub in "[a-z0-9-]{1,64}",
    ) {
        let kp = generate_key_pair(Algorithm::ES384).unwrap();
        let key_pair = key_pair_from_pkcs8(Algorithm::ES384, kp.private_key_pkcs8.as_bytes()).unwrap();

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let claims = Claims {
            sub: Some(sub.clone()),
            exp: Some(now + 3600),
            iat: Some(now),
            ..Default::default()
        };

        let encoded = sign_jwt(Algorithm::ES384, &kp.kid, &claims, &key_pair).unwrap();

        let verified = verify_with_jsonwebtoken(
            &encoded.token,
            &kp.public_jwk,
            JtAlgorithm::ES384,
        ).unwrap();

        prop_assert_eq!(verified["sub"].as_str(), Some(sub.as_str()));
    }
}
