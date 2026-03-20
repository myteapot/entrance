use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};

const ENCRYPTION_VERSION: &str = "v1";
const KEY_CONTEXT: &[u8] = b"entrance.vault.aes256gcm";

pub struct VaultCipher {
    cipher: Aes256Gcm,
}

impl VaultCipher {
    pub fn from_device() -> Result<Self> {
        let identifier = load_device_identifier()?;
        Self::from_device_identifier(&identifier)
    }

    pub fn from_device_identifier(identifier: &str) -> Result<Self> {
        let mut hasher = Sha256::new();
        hasher.update(KEY_CONTEXT);
        hasher.update(identifier.as_bytes());
        let key = hasher.finalize();
        let cipher = Aes256Gcm::new_from_slice(key.as_slice())
            .context("failed to initialize vault cipher")?;
        Ok(Self { cipher })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| anyhow!("failed to encrypt vault secret"))?;

        Ok(format!(
            "{ENCRYPTION_VERSION}.{}.{}",
            URL_SAFE_NO_PAD.encode(nonce),
            URL_SAFE_NO_PAD.encode(ciphertext)
        ))
    }

    pub fn decrypt(&self, encrypted_value: &str) -> Result<String> {
        let mut segments = encrypted_value.splitn(3, '.');
        let version = segments.next().unwrap_or_default();
        let nonce_segment = segments.next().unwrap_or_default();
        let ciphertext_segment = segments.next().unwrap_or_default();

        if version != ENCRYPTION_VERSION
            || nonce_segment.is_empty()
            || ciphertext_segment.is_empty()
        {
            return Err(anyhow!("invalid encrypted vault payload format"));
        }

        let nonce_bytes = URL_SAFE_NO_PAD
            .decode(nonce_segment)
            .context("failed to decode vault nonce")?;
        if nonce_bytes.len() != 12 {
            return Err(anyhow!("vault nonce has an unexpected length"));
        }

        let ciphertext = URL_SAFE_NO_PAD
            .decode(ciphertext_segment)
            .context("failed to decode vault ciphertext")?;
        let plaintext = self
            .cipher
            .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_ref())
            .map_err(|_| anyhow!("failed to decrypt vault secret"))?;

        String::from_utf8(plaintext).context("vault secret is not valid UTF-8")
    }
}

#[cfg(target_os = "windows")]
fn load_device_identifier() -> Result<String> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let cryptography = hklm
        .open_subkey("SOFTWARE\\Microsoft\\Cryptography")
        .context("failed to open MachineGuid registry key")?;
    let machine_guid: String = cryptography
        .get_value("MachineGuid")
        .context("failed to read MachineGuid from registry")?;

    Ok(format!("windows:{machine_guid}"))
}

#[cfg(target_os = "linux")]
fn load_device_identifier() -> Result<String> {
    for path in ["/etc/machine-id", "/var/lib/dbus/machine-id"] {
        if let Ok(value) = std::fs::read_to_string(path) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(format!("linux:{trimmed}"));
            }
        }
    }

    fallback_device_identifier()
}

#[cfg(target_os = "macos")]
fn load_device_identifier() -> Result<String> {
    let output = std::process::Command::new("ioreg")
        .args(["-rd1", "-c", "IOPlatformExpertDevice"])
        .output()
        .context("failed to query IOPlatformUUID")?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if let Some((_, value)) = line.split_once("IOPlatformUUID") {
            let trimmed = value
                .split('=')
                .nth(1)
                .map(str::trim)
                .map(|value| value.trim_matches('"'))
                .unwrap_or_default();
            if !trimmed.is_empty() {
                return Ok(format!("macos:{trimmed}"));
            }
        }
    }

    fallback_device_identifier()
}

#[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
fn load_device_identifier() -> Result<String> {
    fallback_device_identifier()
}

#[cfg(not(target_os = "windows"))]
fn fallback_device_identifier() -> Result<String> {
    for key in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(format!("fallback:{trimmed}"));
            }
        }
    }

    Err(anyhow!(
        "unable to determine a device identifier for vault encryption"
    ))
}

#[cfg(test)]
mod tests {
    use super::VaultCipher;

    #[test]
    fn encrypts_and_decrypts_round_trip() {
        let cipher = VaultCipher::from_device_identifier("test-device")
            .expect("test cipher should initialize");
        let encrypted = cipher.encrypt("secret-value").expect("should encrypt");
        let decrypted = cipher.decrypt(&encrypted).expect("should decrypt");

        assert_eq!(decrypted, "secret-value");
    }
}
