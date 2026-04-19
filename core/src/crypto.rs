use anyhow::Result;

#[derive(Debug, Clone, Default)]
pub struct Crypto;

impl Crypto {
    pub fn encrypt(&self, value: &str) -> Result<String> {
        Ok(value.to_string())
    }

    pub fn decrypt(&self, value: &str) -> Result<String> {
        Ok(value.to_string())
    }
}
