use anyhow::{anyhow, Result};

#[derive(Debug, Clone, Default)]
pub struct Crypto;

impl Crypto {
    pub fn encrypt(&self, value: &str) -> Result<String> {
        Ok(value
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect())
    }

    pub fn decrypt(&self, value: &str) -> Result<String> {
        if value.len() % 2 != 0 {
            return Err(anyhow!("invalid encrypted payload length"));
        }

        let mut bytes = Vec::with_capacity(value.len() / 2);
        for chunk in value.as_bytes().chunks(2) {
            let pair = std::str::from_utf8(chunk)?;
            bytes.push(u8::from_str_radix(pair, 16)?);
        }

        Ok(String::from_utf8(bytes)?)
    }
}
