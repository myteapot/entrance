use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::{secret_tag, storage::DrawerStorage};
use entrance_core::{Crypto, DrawerFilter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSecret {
    pub title: String,
    pub secret: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSecretRecord {
    pub id: i64,
    pub title: String,
    pub encrypted: bool,
}

pub fn store_secret(
    storage: &DrawerStorage,
    crypto: &Crypto,
    mut secret: VaultSecret,
) -> Result<VaultSecretRecord> {
    if !secret.tags.iter().any(|tag| tag == secret_tag()) {
        secret.tags.push(secret_tag().to_string());
    }

    let body = crypto.encrypt(&secret.secret)?;
    let id = storage.create_record(secret.title.clone(), body, "vault".to_string(), secret.tags, true)?;
    Ok(VaultSecretRecord {
        id,
        title: secret.title,
        encrypted: true,
    })
}

pub fn list_secrets(storage: &DrawerStorage) -> Result<Vec<VaultSecretRecord>> {
    Ok(storage
        .list(DrawerFilter {
            kind: Some("vault".to_string()),
            tag: None,
        })?
        .into_iter()
        .map(|entry| VaultSecretRecord {
            id: entry.id,
            title: entry.title,
            encrypted: entry.encrypted,
        })
        .collect())
}
