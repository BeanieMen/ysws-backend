use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit, OsRng, rand_core::RngCore},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

#[derive(Clone)]
pub struct TokenCipher {
    key: [u8; 32],
}

impl TokenCipher {
    #[must_use]
    pub const fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Encrypts a plaintext string.
    ///
    /// # Errors
    ///
    /// Returns an error if key initialization or encryption fails.
    pub fn encrypt(&self, plaintext: &str) -> anyhow::Result<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)?;
        let mut nonce = [0_u8; 12];
        OsRng.fill_bytes(&mut nonce);
        let encrypted = cipher
            .encrypt(Nonce::from_slice(&nonce), plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("failed to encrypt Hackatime token"))?;
        Ok(format!(
            "v1.{}.{}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(encrypted)
        ))
    }

    /// Decrypts an encrypted token string.
    ///
    /// # Errors
    ///
    /// Returns an error if format validation, base64 decoding, key initialization, or decryption fails.
    pub fn decrypt(&self, value: &str) -> anyhow::Result<String> {
        let mut parts = value.split('.');
        if parts.next() != Some("v1") {
            anyhow::bail!("unsupported encrypted-token version")
        }
        let nonce = URL_SAFE_NO_PAD.decode(
            parts
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing token nonce"))?,
        )?;
        let ciphertext = URL_SAFE_NO_PAD.decode(
            parts
                .next()
                .ok_or_else(|| anyhow::anyhow!("missing token ciphertext"))?,
        )?;
        if parts.next().is_some() || nonce.len() != 12 {
            anyhow::bail!("invalid encrypted token")
        }
        let cipher = Aes256Gcm::new_from_slice(&self.key)?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("could not decrypt stored Hackatime token"))?;
        Ok(String::from_utf8(plaintext)?)
    }
}

#[cfg(test)]
mod tests {
    use super::TokenCipher;

    #[test]
    fn round_trips_and_uses_a_random_nonce() {
        let cipher = TokenCipher::new([7; 32]);
        let first = cipher.encrypt("secret-token").unwrap();
        let second = cipher.encrypt("secret-token").unwrap();
        assert_ne!(first, second);
        assert_eq!(cipher.decrypt(&first).unwrap(), "secret-token");
    }

    #[test]
    fn rejects_invalid_ciphertext() {
        let cipher = TokenCipher::new([7; 32]);
        assert!(cipher.decrypt("v1.invalid.invalid").is_err());
    }
}
