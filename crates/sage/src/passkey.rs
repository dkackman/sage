use aes_gcm::{aead::Aead, AeadCore, Aes256Gcm, Key, KeyInit, Nonce};
use base64::{engine::general_purpose::STANDARD, Engine};
use rand::{CryptoRng, Rng};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasskeyError {
    #[error("PRF secret must be 32 bytes, got {0}")]
    InvalidPrfSecretLength(usize),
    #[error("failed to encrypt password")]
    Encrypt,
    #[error("failed to decrypt password")]
    Decrypt,
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("wrapped password is truncated")]
    Truncated,
}

pub fn wrap_password(
    prf_secret: &[u8],
    password: &[u8],
    rng: &mut (impl CryptoRng + Rng),
) -> Result<String, PasskeyError> {
    if prf_secret.len() != 32 {
        return Err(PasskeyError::InvalidPrfSecretLength(prf_secret.len()));
    }
    let key = Key::<Aes256Gcm>::from_slice(prf_secret);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut *rng);
    let ciphertext = cipher
        .encrypt(&nonce, password)
        .map_err(|_| PasskeyError::Encrypt)?;
    let mut combined = nonce.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(combined))
}

pub fn unwrap_password(prf_secret: &[u8], wrapped: &str) -> Result<Vec<u8>, PasskeyError> {
    if prf_secret.len() != 32 {
        return Err(PasskeyError::InvalidPrfSecretLength(prf_secret.len()));
    }
    let combined = STANDARD.decode(wrapped)?;
    if combined.len() < 12 {
        return Err(PasskeyError::Truncated);
    }
    let (nonce_bytes, ciphertext) = combined.split_at(12);
    let key = Key::<Aes256Gcm>::from_slice(prf_secret);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| PasskeyError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand::SeedableRng;

    #[test]
    fn test_wrap_unwrap_roundtrip() {
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        let secret = [9u8; 32];
        let wrapped = wrap_password(&secret, b"correct horse", &mut rng).unwrap();
        let out = unwrap_password(&secret, &wrapped).unwrap();
        assert_eq!(out, b"correct horse");
    }

    #[test]
    fn test_wrong_secret_fails() {
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        let wrapped = wrap_password(&[9u8; 32], b"pw", &mut rng).unwrap();
        assert!(matches!(
            unwrap_password(&[8u8; 32], &wrapped),
            Err(PasskeyError::Decrypt)
        ));
    }

    #[test]
    fn test_bad_length_secret_rejected() {
        let mut rng = ChaCha20Rng::from_seed([3u8; 32]);
        assert!(matches!(
            wrap_password(&[0u8; 16], b"pw", &mut rng),
            Err(PasskeyError::InvalidPrfSecretLength(16))
        ));
    }
}
