use aws_lc_rs::rand::SystemRandom;
use aws_lc_rs::signature::{self, EcdsaKeyPair, KeyPair};
use base64ct::{Base64UrlUnpadded, Encoding};
use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::CryptoError;

/// Supported ECDSA signing algorithms (RFC 7518 Section 3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Algorithm {
    /// ECDSA using P-256 and SHA-256
    ES256,
    /// ECDSA using P-384 and SHA-384
    ES384,
}

impl Algorithm {
    /// Returns the JWA algorithm identifier string.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Algorithm::ES256 => "ES256",
            Algorithm::ES384 => "ES384",
        }
    }

    /// Returns the EC curve name (RFC 7518 Section 6.2.1.1).
    #[must_use]
    pub fn curve_name(&self) -> &'static str {
        match self {
            Algorithm::ES256 => "P-256",
            Algorithm::ES384 => "P-384",
        }
    }

    /// Returns the byte length of each EC coordinate for this curve.
    #[must_use]
    pub fn coordinate_size(&self) -> usize {
        match self {
            Algorithm::ES256 => 32,
            Algorithm::ES384 => 48,
        }
    }

    /// Returns the aws-lc-rs signing algorithm.
    ///
    /// Uses the FIXED variant which produces `r || s` concatenation,
    /// matching the JWS requirement in RFC 7518 Section 3.4.
    pub(crate) fn signing_algorithm(self) -> &'static signature::EcdsaSigningAlgorithm {
        match self {
            Algorithm::ES256 => &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            Algorithm::ES384 => &signature::ECDSA_P384_SHA384_FIXED_SIGNING,
        }
    }

    /// Returns the aws-lc-rs verification algorithm.
    pub(crate) fn verification_algorithm(self) -> &'static signature::EcdsaVerificationAlgorithm {
        match self {
            Algorithm::ES256 => &signature::ECDSA_P256_SHA256_FIXED,
            Algorithm::ES384 => &signature::ECDSA_P384_SHA384_FIXED,
        }
    }
}

impl std::str::FromStr for Algorithm {
    type Err = CryptoError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ES256" => Ok(Algorithm::ES256),
            "ES384" => Ok(Algorithm::ES384),
            _ => Err(CryptoError::AlgorithmMismatch),
        }
    }
}

/// Public JWK representation (RFC 7517).
///
/// This is the format exposed in the JWKS endpoint. Only EC key types
/// are currently supported.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Jwk {
    /// Key type (RFC 7517 Section 4.1). Always "EC" for ECDSA keys.
    pub kty: String,
    /// Public key use (RFC 7517 Section 4.2). Always "sig" for signing keys.
    #[serde(rename = "use")]
    pub key_use: String,
    /// Key ID (RFC 7517 Section 4.5). UUID v4.
    pub kid: String,
    /// Algorithm (RFC 7517 Section 4.4). E.g. "ES256".
    pub alg: String,
    /// EC curve name (RFC 7518 Section 6.2.1.1). E.g. "P-256".
    pub crv: String,
    /// Base64url-encoded x coordinate (RFC 7518 Section 6.2.1.2).
    pub x: String,
    /// Base64url-encoded y coordinate (RFC 7518 Section 6.2.1.3).
    pub y: String,
}

/// JWKS response (RFC 7517 Section 5).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwkSet {
    pub keys: Vec<Jwk>,
}

/// Private key material, zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct PrivateKeyBytes(Vec<u8>);

impl PrivateKeyBytes {
    #[must_use]
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

/// A generated key pair: the PKCS#8 DER private key and the public JWK.
pub struct GeneratedKeyPair {
    pub kid: String,
    pub algorithm: Algorithm,
    pub private_key_pkcs8: PrivateKeyBytes,
    pub public_jwk: Jwk,
}

/// Generate a new ECDSA key pair.
///
/// Returns the PKCS#8 DER-encoded private key and the corresponding
/// public JWK with a fresh UUID v4 `kid`.
///
/// # Errors
///
/// Returns `CryptoError` if key generation or public key extraction fails.
pub fn generate_key_pair(algorithm: Algorithm) -> Result<GeneratedKeyPair, CryptoError> {
    let rng = SystemRandom::new();

    let pkcs8_doc = EcdsaKeyPair::generate_pkcs8(algorithm.signing_algorithm(), &rng)
        .map_err(|_| CryptoError::KeyGeneration)?;

    let pkcs8_bytes = pkcs8_doc.as_ref().to_vec();

    let kid = uuid::Uuid::new_v4().to_string();
    let public_jwk = public_jwk_from_pkcs8(&kid, algorithm, &pkcs8_bytes)?;

    Ok(GeneratedKeyPair {
        kid,
        algorithm,
        private_key_pkcs8: PrivateKeyBytes::new(pkcs8_bytes),
        public_jwk,
    })
}

/// Reconstruct a public JWK from a PKCS#8 DER private key.
///
/// Parses the EC uncompressed point (0x04 || x || y) from the public key
/// and base64url-encodes the coordinates per RFC 7518 Section 6.2.1.
///
/// # Errors
///
/// Returns `CryptoError::InvalidKeyMaterial` if the PKCS#8 data is invalid.
pub fn public_jwk_from_pkcs8(
    kid: &str,
    algorithm: Algorithm,
    pkcs8_der: &[u8],
) -> Result<Jwk, CryptoError> {
    let key_pair = EcdsaKeyPair::from_pkcs8(algorithm.signing_algorithm(), pkcs8_der)
        .map_err(|_| CryptoError::InvalidKeyMaterial)?;

    let public_key_bytes = key_pair.public_key().as_ref();
    let coord_size = algorithm.coordinate_size();

    // Uncompressed EC point format: 0x04 || x || y
    let expected_len = 1 + 2 * coord_size;
    if public_key_bytes.len() != expected_len || public_key_bytes[0] != 0x04 {
        return Err(CryptoError::InvalidKeyMaterial);
    }

    let x = &public_key_bytes[1..=coord_size];
    let y = &public_key_bytes[1 + coord_size..];

    Ok(Jwk {
        kty: "EC".to_string(),
        key_use: "sig".to_string(),
        kid: kid.to_string(),
        alg: algorithm.as_str().to_string(),
        crv: algorithm.curve_name().to_string(),
        x: Base64UrlUnpadded::encode_string(x),
        y: Base64UrlUnpadded::encode_string(y),
    })
}

/// Build a `JwkSet` from a list of public JWKs.
#[must_use]
pub fn build_jwk_set(keys: Vec<Jwk>) -> JwkSet {
    JwkSet { keys }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_es256_key_pair() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        assert_eq!(kp.algorithm, Algorithm::ES256);
        assert_eq!(kp.public_jwk.kty, "EC");
        assert_eq!(kp.public_jwk.alg, "ES256");
        assert_eq!(kp.public_jwk.crv, "P-256");
        assert_eq!(kp.public_jwk.key_use, "sig");
        // kid is a UUID v4 (36 chars with hyphens)
        assert_eq!(kp.kid.len(), 36);
        assert_eq!(kp.public_jwk.kid, kp.kid);
    }

    #[test]
    fn generate_es384_key_pair() {
        let kp = generate_key_pair(Algorithm::ES384).unwrap();
        assert_eq!(kp.algorithm, Algorithm::ES384);
        assert_eq!(kp.public_jwk.kty, "EC");
        assert_eq!(kp.public_jwk.alg, "ES384");
        assert_eq!(kp.public_jwk.crv, "P-384");
        assert_eq!(kp.public_jwk.key_use, "sig");
        assert_eq!(kp.kid.len(), 36);
    }

    #[test]
    fn jwk_serialization_matches_rfc7517() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let json = serde_json::to_value(&kp.public_jwk).unwrap();

        // All required fields present
        assert!(json.get("kty").is_some());
        assert!(json.get("use").is_some());
        assert!(json.get("kid").is_some());
        assert!(json.get("alg").is_some());
        assert!(json.get("crv").is_some());
        assert!(json.get("x").is_some());
        assert!(json.get("y").is_some());

        // base64url without padding
        assert!(!kp.public_jwk.x.contains('='));
        assert!(!kp.public_jwk.y.contains('='));
    }

    #[test]
    fn es256_coordinate_sizes() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        // P-256: 32-byte coordinates -> 43 base64url chars (32 * 4/3 = 42.67, ceil = 43)
        let x_bytes = Base64UrlUnpadded::decode_vec(&kp.public_jwk.x).unwrap();
        let y_bytes = Base64UrlUnpadded::decode_vec(&kp.public_jwk.y).unwrap();
        assert_eq!(x_bytes.len(), 32);
        assert_eq!(y_bytes.len(), 32);
    }

    #[test]
    fn es384_coordinate_sizes() {
        let kp = generate_key_pair(Algorithm::ES384).unwrap();
        // P-384: 48-byte coordinates
        let x_bytes = Base64UrlUnpadded::decode_vec(&kp.public_jwk.x).unwrap();
        let y_bytes = Base64UrlUnpadded::decode_vec(&kp.public_jwk.y).unwrap();
        assert_eq!(x_bytes.len(), 48);
        assert_eq!(y_bytes.len(), 48);
    }

    #[test]
    fn public_jwk_from_pkcs8_roundtrip() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let reconstructed =
            public_jwk_from_pkcs8(&kp.kid, Algorithm::ES256, kp.private_key_pkcs8.as_bytes())
                .unwrap();
        assert_eq!(reconstructed.x, kp.public_jwk.x);
        assert_eq!(reconstructed.y, kp.public_jwk.y);
    }

    #[test]
    fn jwk_set_serialization() {
        let kp1 = generate_key_pair(Algorithm::ES256).unwrap();
        let kp2 = generate_key_pair(Algorithm::ES384).unwrap();
        let set = build_jwk_set(vec![kp1.public_jwk, kp2.public_jwk]);
        let json = serde_json::to_string(&set).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["keys"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn jwk_deserialization_roundtrip() {
        let kp = generate_key_pair(Algorithm::ES256).unwrap();
        let json = serde_json::to_string(&kp.public_jwk).unwrap();
        let deserialized: Jwk = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, kp.public_jwk);
    }

    #[test]
    fn algorithm_from_str() {
        assert_eq!("ES256".parse(), Ok(Algorithm::ES256));
        assert_eq!("ES384".parse(), Ok(Algorithm::ES384));
        assert!("RS256".parse::<Algorithm>().is_err());
        assert!("none".parse::<Algorithm>().is_err());
    }
}
