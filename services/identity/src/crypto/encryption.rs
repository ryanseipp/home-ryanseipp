use aws_lc_rs::aead::{AES_256_GCM, Aad, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};

use crate::crypto::CryptoError;

/// Expected KEK length for AES-256: 32 bytes.
const KEK_LEN: usize = 32;

/// AES-GCM tag length: 16 bytes (128 bits).
const TAG_LEN: usize = 16;

/// Encrypted key material.
///
/// Format: `[nonce (12 bytes)][ciphertext][authentication tag (16 bytes)]`
///
/// The nonce is not secret and is prepended to the ciphertext for
/// self-contained storage.
#[derive(Debug, Clone)]
pub struct EncryptedKey {
    data: Vec<u8>,
}

impl EncryptedKey {
    /// Construct from raw bytes, validating minimum length.
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, CryptoError> {
        // Minimum: nonce + at least 1 byte ciphertext + tag
        if data.len() < NONCE_LEN + 1 + TAG_LEN {
            return Err(CryptoError::InvalidKeyMaterial);
        }
        Ok(Self { data })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    fn nonce_bytes(&self) -> &[u8] {
        &self.data[..NONCE_LEN]
    }

    fn ciphertext_and_tag(&self) -> &[u8] {
        &self.data[NONCE_LEN..]
    }
}

/// Encrypt private key material using AES-256-GCM.
///
/// - `kek`: Key Encryption Key, must be exactly 32 bytes.
/// - `plaintext`: Private key bytes to encrypt.
/// - `aad`: Additional Authenticated Data (e.g. the `kid`), binding the
///   ciphertext to a specific key identity as defense-in-depth.
///
/// Returns `EncryptedKey` containing `nonce || ciphertext || tag`.
/// A fresh random 96-bit nonce is generated for each call.
pub fn encrypt_private_key(
    kek: &[u8],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<EncryptedKey, CryptoError> {
    if kek.len() != KEK_LEN {
        return Err(CryptoError::InvalidKek);
    }

    let unbound_key = UnboundKey::new(&AES_256_GCM, kek).map_err(|_| CryptoError::InvalidKek)?;
    let key = LessSafeKey::new(unbound_key);

    let mut nonce_bytes = [0u8; NONCE_LEN];
    aws_lc_rs::rand::fill(&mut nonce_bytes).map_err(|_| CryptoError::Encryption)?;

    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = plaintext.to_vec();
    key.seal_in_place_append_tag(nonce, Aad::from(aad), &mut in_out)
        .map_err(|_| CryptoError::Encryption)?;

    // Prepend nonce: [nonce || ciphertext || tag]
    let mut data = Vec::with_capacity(NONCE_LEN + in_out.len());
    data.extend_from_slice(&nonce_bytes);
    data.extend_from_slice(&in_out);

    Ok(EncryptedKey { data })
}

/// Decrypt private key material using AES-256-GCM.
///
/// - `kek`: Key Encryption Key, must be exactly 32 bytes.
/// - `encrypted`: The `EncryptedKey` (nonce || ciphertext || tag).
/// - `aad`: The same Additional Authenticated Data used during encryption.
///
/// Returns the plaintext private key bytes. Fails with `CryptoError::Decryption`
/// if the KEK is wrong, the data was tampered with, or the AAD doesn't match.
pub fn decrypt_private_key(
    kek: &[u8],
    encrypted: &EncryptedKey,
    aad: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if kek.len() != KEK_LEN {
        return Err(CryptoError::InvalidKek);
    }

    let unbound_key = UnboundKey::new(&AES_256_GCM, kek).map_err(|_| CryptoError::InvalidKek)?;
    let key = LessSafeKey::new(unbound_key);

    let nonce_bytes: [u8; NONCE_LEN] = encrypted
        .nonce_bytes()
        .try_into()
        .map_err(|_| CryptoError::Decryption)?;
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);

    let mut in_out = encrypted.ciphertext_and_tag().to_vec();
    let plaintext = key
        .open_in_place(nonce, Aad::from(aad), &mut in_out)
        .map_err(|_| CryptoError::Decryption)?;

    Ok(plaintext.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_kek() -> Vec<u8> {
        let mut kek = vec![0u8; KEK_LEN];
        aws_lc_rs::rand::fill(&mut kek).unwrap();
        kek
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let kek = random_kek();
        let plaintext = b"this is a PKCS#8 private key (simulated)";
        let aad = b"test-kid-123";

        let encrypted = encrypt_private_key(&kek, plaintext, aad).unwrap();
        let decrypted = decrypt_private_key(&kek, &encrypted, aad).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn wrong_kek_fails_decryption() {
        let kek1 = random_kek();
        let kek2 = random_kek();
        let plaintext = b"secret key material";
        let aad = b"kid";

        let encrypted = encrypt_private_key(&kek1, plaintext, aad).unwrap();
        let result = decrypt_private_key(&kek2, &encrypted, aad);

        assert!(matches!(result, Err(CryptoError::Decryption)));
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let kek = random_kek();
        let plaintext = b"secret key material";
        let aad = b"kid";

        let encrypted = encrypt_private_key(&kek, plaintext, aad).unwrap();
        let mut tampered = encrypted.as_bytes().to_vec();
        // Flip a bit in the ciphertext (after the nonce)
        tampered[NONCE_LEN + 1] ^= 0xFF;
        let tampered_enc = EncryptedKey::from_bytes(tampered).unwrap();

        let result = decrypt_private_key(&kek, &tampered_enc, aad);
        assert!(matches!(result, Err(CryptoError::Decryption)));
    }

    #[test]
    fn wrong_aad_fails_decryption() {
        let kek = random_kek();
        let plaintext = b"secret key material";

        let encrypted = encrypt_private_key(&kek, plaintext, b"kid-1").unwrap();
        let result = decrypt_private_key(&kek, &encrypted, b"kid-2");

        assert!(matches!(result, Err(CryptoError::Decryption)));
    }

    #[test]
    fn invalid_kek_length_rejected() {
        let short_kek = vec![0u8; 16]; // AES-128, not AES-256
        let result = encrypt_private_key(&short_kek, b"data", b"");
        assert!(matches!(result, Err(CryptoError::InvalidKek)));
    }

    #[test]
    fn encrypted_key_minimum_length_enforced() {
        // Too short to contain nonce + 1 byte + tag
        let too_short = vec![0u8; NONCE_LEN + TAG_LEN]; // missing ciphertext byte
        assert!(EncryptedKey::from_bytes(too_short).is_err());
    }

    #[test]
    fn each_encryption_produces_unique_nonce() {
        let kek = random_kek();
        let plaintext = b"same plaintext";
        let aad = b"kid";

        let enc1 = encrypt_private_key(&kek, plaintext, aad).unwrap();
        let enc2 = encrypt_private_key(&kek, plaintext, aad).unwrap();

        // Different nonces -> different ciphertexts
        assert_ne!(enc1.as_bytes(), enc2.as_bytes());
        // But both decrypt to the same plaintext
        assert_eq!(
            decrypt_private_key(&kek, &enc1, aad).unwrap(),
            decrypt_private_key(&kek, &enc2, aad).unwrap()
        );
    }
}
