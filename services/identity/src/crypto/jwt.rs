use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{EcdsaKeyPair, UnparsedPublicKey};
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::{Deserialize, Serialize};

use crate::crypto::CryptoError;
use crate::crypto::jwk::{Algorithm, Jwk};

/// JWT JOSE header (RFC 7515 Section 4).
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtHeader {
    /// Algorithm (MUST be set per RFC 8725).
    pub alg: String,
    /// Token type.
    pub typ: String,
    /// Key ID — identifies which key signed this token.
    pub kid: String,
}

/// Audience can be a single string or an array of strings (RFC 7519 Section 4.1.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Audience {
    Single(String),
    Multiple(Vec<String>),
}

impl Audience {
    /// Check whether the audience contains the given value.
    #[must_use]
    pub fn contains(&self, value: &str) -> bool {
        match self {
            Audience::Single(s) => s == value,
            Audience::Multiple(v) => v.iter().any(|s| s == value),
        }
    }
}

/// Standard JWT claims (RFC 7519 Section 4.1).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// Issuer (Section 4.1.1)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,
    /// Subject (Section 4.1.2)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    /// Audience (Section 4.1.3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aud: Option<Audience>,
    /// Expiration time as `NumericDate` — seconds since epoch (Section 4.1.4)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
    /// Not before as `NumericDate` (Section 4.1.5)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>,
    /// Issued at as `NumericDate` (Section 4.1.6)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat: Option<u64>,
    /// JWT ID (Section 4.1.7)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,

    // --- OIDC standard claims ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_username: Option<String>,

    /// Additional custom claims.
    #[serde(flatten)]
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

/// An encoded JWT (compact serialization: header.payload.signature).
pub struct EncodedJwt {
    pub token: String,
    pub kid: String,
}

/// Verification options for JWT validation (RFC 8725 best practices).
pub struct VerificationOptions<'a> {
    /// Allowed algorithms — MUST be an explicit allowlist.
    /// Never trust the token's `alg` header alone.
    pub allowed_algorithms: &'a [Algorithm],
    /// Expected issuer. If set, the token's `iss` claim must match.
    pub issuer: Option<&'a str>,
    /// Expected audience. If set, the token's `aud` claim must contain this value.
    pub audience: Option<&'a str>,
    /// Current time in seconds since epoch (for exp/nbf validation).
    pub current_time: u64,
    /// Clock skew tolerance in seconds.
    pub leeway: u64,
}

/// Sign claims into a JWT using the given ECDSA key pair.
///
/// Produces compact serialization: `base64url(header).base64url(payload).base64url(signature)`
///
/// ES256 uses `ECDSA_P256_SHA256_FIXED_SIGNING` which produces `r || s` (64 bytes),
/// matching the JWS requirement in RFC 7518 Section 3.4.
///
/// # Errors
///
/// Returns `CryptoError` if serialization or signing fails.
pub fn sign_jwt(
    algorithm: Algorithm,
    kid: &str,
    claims: &Claims,
    key_pair: &EcdsaKeyPair,
) -> Result<EncodedJwt, CryptoError> {
    let header = JwtHeader {
        alg: algorithm.as_str().to_string(),
        typ: "JWT".to_string(),
        kid: kid.to_string(),
    };

    let header_json = serde_json::to_vec(&header).map_err(|_| CryptoError::Serialization)?;
    let claims_json = serde_json::to_vec(claims).map_err(|_| CryptoError::Serialization)?;

    let header_b64 = Base64UrlUnpadded::encode_string(&header_json);
    let claims_b64 = Base64UrlUnpadded::encode_string(&claims_json);

    let signing_input = format!("{header_b64}.{claims_b64}");

    let rng = SystemRandom::new();
    let signature = key_pair
        .sign(&rng, signing_input.as_bytes())
        .map_err(|_| CryptoError::Signing)?;

    let sig_b64 = Base64UrlUnpadded::encode_string(signature.as_ref());

    Ok(EncodedJwt {
        token: format!("{signing_input}.{sig_b64}"),
        kid: kid.to_string(),
    })
}

/// Verify a JWT signature and validate claims.
///
/// Per RFC 8725:
/// 1. Split token, decode header
/// 2. Validate `alg` against allowlist (never trust header alone)
/// 3. Verify signature using the public key
/// 4. Validate exp, nbf, iss, aud claims
///
/// # Errors
///
/// Returns `CryptoError` if the token is malformed, the signature is invalid, or claims fail validation.
pub fn verify_jwt(
    token: &str,
    public_jwk: &Jwk,
    options: &VerificationOptions<'_>,
) -> Result<Claims, CryptoError> {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(CryptoError::InvalidToken);
    }

    let header_bytes =
        Base64UrlUnpadded::decode_vec(parts[0]).map_err(|_| CryptoError::InvalidToken)?;
    let header: JwtHeader =
        serde_json::from_slice(&header_bytes).map_err(|_| CryptoError::InvalidToken)?;

    // RFC 8725: validate alg against explicit allowlist
    let algorithm: Algorithm = header.alg.parse()?;
    if !options.allowed_algorithms.contains(&algorithm) {
        return Err(CryptoError::AlgorithmMismatch);
    }

    // Validate kid matches the provided public key
    if header.kid != public_jwk.kid {
        return Err(CryptoError::Verification);
    }

    // Reconstruct public key from JWK coordinates
    let x_bytes = Base64UrlUnpadded::decode_vec(&public_jwk.x)
        .map_err(|_| CryptoError::InvalidKeyMaterial)?;
    let y_bytes = Base64UrlUnpadded::decode_vec(&public_jwk.y)
        .map_err(|_| CryptoError::InvalidKeyMaterial)?;

    // Uncompressed EC point: 0x04 || x || y
    let mut public_key_bytes = Vec::with_capacity(1 + x_bytes.len() + y_bytes.len());
    public_key_bytes.push(0x04);
    public_key_bytes.extend_from_slice(&x_bytes);
    public_key_bytes.extend_from_slice(&y_bytes);

    let public_key = UnparsedPublicKey::new(algorithm.verification_algorithm(), &public_key_bytes);

    // Verify signature
    let signing_input = format!("{}.{}", parts[0], parts[1]);
    let signature_bytes =
        Base64UrlUnpadded::decode_vec(parts[2]).map_err(|_| CryptoError::InvalidToken)?;

    public_key
        .verify(signing_input.as_bytes(), &signature_bytes)
        .map_err(|_| CryptoError::Verification)?;

    // Decode and validate claims
    let claims_bytes =
        Base64UrlUnpadded::decode_vec(parts[1]).map_err(|_| CryptoError::InvalidToken)?;
    let claims: Claims =
        serde_json::from_slice(&claims_bytes).map_err(|_| CryptoError::InvalidToken)?;

    // Validate exp (RFC 7519 Section 4.1.4)
    if let Some(exp) = claims.exp
        && options.current_time > exp + options.leeway
    {
        return Err(CryptoError::TokenExpired);
    }

    // Validate nbf (RFC 7519 Section 4.1.5)
    if let Some(nbf) = claims.nbf
        && options.current_time + options.leeway < nbf
    {
        return Err(CryptoError::InvalidToken);
    }

    // Validate iss (RFC 7519 Section 4.1.1)
    if let Some(expected_iss) = options.issuer {
        match &claims.iss {
            Some(iss) if iss == expected_iss => {}
            _ => return Err(CryptoError::InvalidToken),
        }
    }

    // Validate aud (RFC 7519 Section 4.1.3)
    if let Some(expected_aud) = options.audience {
        match &claims.aud {
            Some(aud) if aud.contains(expected_aud) => {}
            _ => return Err(CryptoError::InvalidToken),
        }
    }

    Ok(claims)
}

/// Reconstruct an `EcdsaKeyPair` from PKCS#8 DER bytes.
///
/// # Errors
///
/// Returns `CryptoError::InvalidKeyMaterial` if the PKCS#8 data is invalid.
pub fn key_pair_from_pkcs8(
    algorithm: Algorithm,
    pkcs8_der: &[u8],
) -> Result<EcdsaKeyPair, CryptoError> {
    EcdsaKeyPair::from_pkcs8(algorithm.signing_algorithm(), pkcs8_der)
        .map_err(|_| CryptoError::InvalidKeyMaterial)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::jwk::generate_key_pair;

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    }

    fn sample_claims(exp: u64) -> Claims {
        Claims {
            iss: Some("https://home.ryanseipp.com".into()),
            sub: Some("user-123".into()),
            aud: Some(Audience::Single("https://api.home.ryanseipp.com".into())),
            exp: Some(exp),
            iat: Some(now_secs()),
            jti: Some(uuid::Uuid::new_v4().to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn sign_and_verify_roundtrip_es256() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() + 3600);
        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        assert_eq!(encoded.token.split('.').count(), 3);

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: Some("https://home.ryanseipp.com"),
            audience: Some("https://api.home.ryanseipp.com"),
            current_time: now_secs(),
            leeway: 0,
        };

        let verified = verify_jwt(&encoded.token, &kp.public_jwk, &options).unwrap();
        assert_eq!(verified.sub, claims.sub);
        assert_eq!(verified.iss, claims.iss);
    }

    #[test]
    fn sign_and_verify_roundtrip_es384() {
        let kp = generate_key_pair(Algorithm::ES384).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES384, kp.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() + 3600);
        let encoded = sign_jwt(Algorithm::ES384, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES384],
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        let verified = verify_jwt(&encoded.token, &kp.public_jwk, &options).unwrap();
        assert_eq!(verified.sub, claims.sub);
    }

    #[test]
    fn expired_token_rejected() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() - 100);
        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        let result = verify_jwt(&encoded.token, &kp.public_jwk, &options);
        assert!(matches!(result, Err(CryptoError::TokenExpired)));
    }

    #[test]
    fn leeway_allows_slightly_expired_token() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        // Expired 30 seconds ago
        let claims = sample_claims(now_secs() - 30);
        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 60, // 60 seconds leeway
        };

        // Should succeed because 30s expired < 60s leeway
        assert!(verify_jwt(&encoded.token, &kp.public_jwk, &options).is_ok());
    }

    #[test]
    fn invalid_signature_rejected() {
        let kp1 = generate_key_pair(Algorithm::ES256).unwrap();
        let kp2 = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp1.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() + 3600);
        let encoded = sign_jwt(Algorithm::ES256, &kp1.kid, &claims, &key_pair).unwrap();

        // Verify with wrong public key (but fix kid to bypass kid check)
        let mut wrong_jwk = kp2.public_jwk.clone();
        wrong_jwk.kid = kp1.kid.clone();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        let result = verify_jwt(&encoded.token, &wrong_jwk, &options);
        assert!(matches!(result, Err(CryptoError::Verification)));
    }

    #[test]
    fn algorithm_not_in_allowlist_rejected() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() + 3600);
        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES384], // ES256 not allowed
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        let result = verify_jwt(&encoded.token, &kp.public_jwk, &options);
        assert!(matches!(result, Err(CryptoError::AlgorithmMismatch)));
    }

    #[test]
    fn issuer_mismatch_rejected() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() + 3600);
        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: Some("https://wrong-issuer.com"),
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        let result = verify_jwt(&encoded.token, &kp.public_jwk, &options);
        assert!(matches!(result, Err(CryptoError::InvalidToken)));
    }

    #[test]
    fn audience_mismatch_rejected() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let claims = sample_claims(now_secs() + 3600);
        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: Some("https://wrong-audience.com"),
            current_time: now_secs(),
            leeway: 0,
        };

        let result = verify_jwt(&encoded.token, &kp.public_jwk, &options);
        assert!(matches!(result, Err(CryptoError::InvalidToken)));
    }

    #[test]
    fn multiple_audience_values() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let mut claims = sample_claims(now_secs() + 3600);
        claims.aud = Some(Audience::Multiple(vec![
            "https://api1.example.com".into(),
            "https://api2.example.com".into(),
        ]));

        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: Some("https://api2.example.com"),
            current_time: now_secs(),
            leeway: 0,
        };

        assert!(verify_jwt(&encoded.token, &kp.public_jwk, &options).is_ok());
    }

    #[test]
    fn malformed_token_rejected() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        // Not enough parts
        assert!(matches!(
            verify_jwt("abc.def", &kp.public_jwk, &options),
            Err(CryptoError::InvalidToken)
        ));

        // Too many parts
        assert!(matches!(
            verify_jwt("a.b.c.d", &kp.public_jwk, &options),
            Err(CryptoError::InvalidToken)
        ));
    }

    #[test]
    fn nbf_in_future_rejected() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let key_pair =
            key_pair_from_pkcs8(Algorithm::ES256, kp.private_key_pkcs8.as_bytes()).unwrap();

        let mut claims = sample_claims(now_secs() + 3600);
        claims.nbf = Some(now_secs() + 600); // Not valid for another 10 minutes

        let encoded = sign_jwt(Algorithm::ES256, &kp.kid, &claims, &key_pair).unwrap();

        let options = VerificationOptions {
            allowed_algorithms: &[Algorithm::ES256],
            issuer: None,
            audience: None,
            current_time: now_secs(),
            leeway: 0,
        };

        assert!(matches!(
            verify_jwt(&encoded.token, &kp.public_jwk, &options),
            Err(CryptoError::InvalidToken)
        ));
    }
}
