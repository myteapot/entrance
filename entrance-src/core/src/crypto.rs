use std::{fs, path::Path};

use aes_gcm::{
    aead::{rand_core::RngCore, Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Context, Result};

#[derive(Debug, Clone)]
pub struct Crypto {
    key: [u8; 32],
}

impl Crypto {
    pub fn load_or_create(key_path: impl AsRef<Path>) -> Result<Self> {
        let key_path = key_path.as_ref();
        if key_path.exists() {
            let content = fs::read_to_string(key_path)
                .with_context(|| format!("failed to read vault key at {}", key_path.display()))?;
            return Ok(Self {
                key: decode_fixed_hex::<32>(content.trim())?,
            });
        }

        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);

        if let Some(parent) = key_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(key_path, encode_hex(&key))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = fs::metadata(key_path)?.permissions();
            permissions.set_mode(0o600);
            fs::set_permissions(key_path, permissions)?;
        }

        Ok(Self { key })
    }

    pub fn encrypt(&self, value: &str) -> Result<String> {
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|_| anyhow!("invalid vault key length"))?;
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let ciphertext = cipher
            .encrypt(Nonce::from_slice(&nonce_bytes), value.as_bytes())
            .map_err(|_| anyhow!("failed to encrypt secret"))?;
        Ok(format!(
            "v1:{}:{}",
            encode_hex(&nonce_bytes),
            encode_hex(&ciphertext)
        ))
    }

    pub fn decrypt(&self, value: &str) -> Result<String> {
        let parts = value.split(':').collect::<Vec<_>>();
        if parts.len() != 3 || parts[0] != "v1" {
            return Err(anyhow!("unsupported encrypted payload format"));
        }

        let nonce = decode_fixed_hex::<12>(parts[1])?;
        let ciphertext = decode_hex(parts[2])?;
        let cipher = Aes256Gcm::new_from_slice(&self.key)
            .map_err(|_| anyhow!("invalid vault key length"))?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
            .map_err(|_| anyhow!("failed to decrypt secret"))?;

        Ok(String::from_utf8(plaintext)?)
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_fixed_hex<const N: usize>(value: &str) -> Result<[u8; N]> {
    let bytes = decode_hex(value)?;
    if bytes.len() != N {
        return Err(anyhow!("expected {N} bytes of hex data"));
    }

    let mut out = [0u8; N];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if value.len() % 2 != 0 {
        return Err(anyhow!("invalid hex payload length"));
    }

    let mut bytes = Vec::with_capacity(value.len() / 2);
    for chunk in value.as_bytes().chunks(2) {
        let pair = std::str::from_utf8(chunk)?;
        bytes.push(u8::from_str_radix(pair, 16)?);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::Crypto;

    #[test]
    fn encrypt_round_trips_without_plain_hex_payload() {
        let crypto = Crypto { key: [7; 32] };
        let encrypted = crypto.encrypt("secret-token").unwrap();

        assert!(encrypted.starts_with("v1:"));
        assert!(!encrypted.contains("7365637265742d746f6b656e"));
        assert_eq!(crypto.decrypt(&encrypted).unwrap(), "secret-token");
    }
}
