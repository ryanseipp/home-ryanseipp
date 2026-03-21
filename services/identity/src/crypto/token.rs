use aws_lc_rs::digest::{self, SHA256};
use aws_lc_rs::rand;
use base64ct::{Base64UrlUnpadded, Encoding};

use super::CryptoError;

/// Generate a random verification token.
///
/// Returns `(cleartext_base64url, sha256_hash_bytes)` where:
/// - `cleartext_base64url` is 32 random bytes encoded as base64url (sent to the user)
/// - `sha256_hash_bytes` is the SHA-256 hash of the raw bytes (stored in DB)
pub fn generate_verification_token() -> Result<(String, Vec<u8>), CryptoError> {
    let mut raw_token = [0u8; 32];
    rand::fill(&mut raw_token).map_err(|_| CryptoError::RngFailure)?;

    let token_string = Base64UrlUnpadded::encode_string(&raw_token);
    let token_hash = digest::digest(&SHA256, &raw_token);
    let token_hash_bytes = token_hash.as_ref().to_vec();

    Ok((token_string, token_hash_bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_produces_base64url_and_hash() {
        let (cleartext, hash) = generate_verification_token().unwrap();
        assert!(!cleartext.is_empty());
        assert_eq!(hash.len(), 32); // SHA-256 output

        // Cleartext should be valid base64url
        let decoded = Base64UrlUnpadded::decode_vec(&cleartext).unwrap();
        assert_eq!(decoded.len(), 32);

        // Hash should be the SHA-256 of the decoded bytes
        let expected_hash = digest::digest(&SHA256, &decoded);
        assert_eq!(hash, expected_hash.as_ref());
    }

    #[test]
    fn each_call_produces_unique_token() {
        let (t1, h1) = generate_verification_token().unwrap();
        let (t2, h2) = generate_verification_token().unwrap();
        assert_ne!(t1, t2);
        assert_ne!(h1, h2);
    }
}
