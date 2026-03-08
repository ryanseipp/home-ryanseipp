use base64ct::{Base64, Encoding};
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::crypto::CryptoError;

/// Expected KEK length for AES-256: 32 bytes.
const KEK_LEN: usize = 32;

/// Environment variable name for hex-encoded KEK.
pub const KEK_ENV_HEX: &str = "IDENTITY_KEK_HEX";

/// Environment variable name for base64-encoded KEK.
pub const KEK_ENV_BASE64: &str = "IDENTITY_KEK_BASE64";

/// Environment variable name for file path containing raw KEK bytes.
pub const KEK_FILE_ENV: &str = "IDENTITY_KEK_FILE";

/// A validated Key Encryption Key, zeroized on drop.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct Kek {
    bytes: Vec<u8>,
}

impl Kek {
    /// Create a Kek from raw bytes, validating length is exactly 32 bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self, CryptoError> {
        if bytes.len() != KEK_LEN {
            return Err(CryptoError::InvalidKek);
        }
        Ok(Self { bytes })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Load KEK from environment, trying in order:
///
/// 1. `IDENTITY_KEK_HEX` — hex-encoded (64 hex chars for 32 bytes)
/// 2. `IDENTITY_KEK_BASE64` — standard base64-encoded
/// 3. `IDENTITY_KEK_FILE` — path to file containing raw 32 bytes
///
/// Returns the first successfully loaded KEK.
pub fn load_kek() -> Result<Kek, CryptoError> {
    if let Ok(hex_str) = std::env::var(KEK_ENV_HEX) {
        let bytes = decode_hex(&hex_str)?;
        return Kek::from_bytes(bytes);
    }

    if let Ok(b64_str) = std::env::var(KEK_ENV_BASE64) {
        let bytes = Base64::decode_vec(&b64_str).map_err(|_| CryptoError::InvalidKek)?;
        return Kek::from_bytes(bytes);
    }

    if let Ok(path) = std::env::var(KEK_FILE_ENV) {
        return load_kek_from_file(std::path::Path::new(&path));
    }

    Err(CryptoError::KekLoad(
        "no KEK configured: set IDENTITY_KEK_HEX, IDENTITY_KEK_BASE64, or IDENTITY_KEK_FILE".into(),
    ))
}

/// Load KEK from a specific file path containing raw bytes.
pub fn load_kek_from_file(path: &std::path::Path) -> Result<Kek, CryptoError> {
    let bytes =
        std::fs::read(path).map_err(|e| CryptoError::KekLoad(format!("read failed: {e}")))?;
    Kek::from_bytes(bytes)
}

/// Decode a hex string into bytes.
fn decode_hex(hex: &str) -> Result<Vec<u8>, CryptoError> {
    let hex = hex.trim();
    if !hex.len().is_multiple_of(2) {
        return Err(CryptoError::InvalidKek);
    }

    hex.as_bytes()
        .chunks(2)
        .map(|pair| {
            let hi = hex_digit(pair[0])?;
            let lo = hex_digit(pair[1])?;
            Ok((hi << 4) | lo)
        })
        .collect()
}

fn hex_digit(b: u8) -> Result<u8, CryptoError> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err(CryptoError::InvalidKek),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kek_from_valid_bytes() {
        let bytes = vec![0xAB; KEK_LEN];
        let kek = Kek::from_bytes(bytes).unwrap();
        assert_eq!(kek.as_bytes().len(), KEK_LEN);
    }

    #[test]
    fn kek_rejects_wrong_length() {
        assert!(Kek::from_bytes(vec![0; 16]).is_err());
        assert!(Kek::from_bytes(vec![0; 31]).is_err());
        assert!(Kek::from_bytes(vec![0; 33]).is_err());
        assert!(Kek::from_bytes(vec![0; 64]).is_err());
    }

    #[test]
    fn decode_hex_valid() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let bytes = decode_hex(hex).unwrap();
        assert_eq!(bytes.len(), 32);
        assert_eq!(bytes[0], 0x01);
        assert_eq!(bytes[1], 0x23);
        assert_eq!(bytes[15], 0xef);
    }

    #[test]
    fn decode_hex_uppercase() {
        let hex = "AABBCCDD";
        let bytes = decode_hex(hex).unwrap();
        assert_eq!(bytes, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    }

    #[test]
    fn decode_hex_invalid_chars() {
        assert!(decode_hex("zzzz").is_err());
    }

    #[test]
    fn decode_hex_odd_length() {
        assert!(decode_hex("abc").is_err());
    }

    #[test]
    fn load_kek_from_hex_env() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        temp_env::with_vars(
            [
                (KEK_ENV_HEX, Some(hex)),
                (KEK_ENV_BASE64, None),
                (KEK_FILE_ENV, None),
            ],
            || {
                let kek = load_kek().unwrap();
                assert_eq!(kek.as_bytes().len(), KEK_LEN);
            },
        );
    }

    #[test]
    fn load_kek_from_base64_env() {
        let bytes = vec![0x42u8; KEK_LEN];
        let b64 = Base64::encode_string(&bytes);
        temp_env::with_vars(
            [
                (KEK_ENV_HEX, None),
                (KEK_ENV_BASE64, Some(b64.as_str())),
                (KEK_FILE_ENV, None),
            ],
            || {
                let kek = load_kek().unwrap();
                assert_eq!(kek.as_bytes(), &bytes);
            },
        );
    }

    #[test]
    fn load_kek_from_file_path() {
        let dir = std::env::temp_dir();
        let path = dir.join("test_kek_file");
        let bytes = vec![0xDE; KEK_LEN];
        std::fs::write(&path, &bytes).unwrap();

        let kek = load_kek_from_file(&path).unwrap();
        assert_eq!(kek.as_bytes(), &bytes);

        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn load_kek_no_config_fails() {
        temp_env::with_vars(
            [
                (KEK_ENV_HEX, None::<&str>),
                (KEK_ENV_BASE64, None),
                (KEK_FILE_ENV, None),
            ],
            || {
                let result = load_kek();
                assert!(matches!(result, Err(CryptoError::KekLoad(_))));
            },
        );
    }
}
