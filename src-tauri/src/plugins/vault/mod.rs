pub mod commands;
pub(crate) mod crypto;

use std::sync::Arc;

use anyhow::Result;

use crate::{
    core::data_store::{
        DataStore, MigrationStep, StoredVaultMcpConfig, StoredVaultToken, StoredVaultTokenSecret,
    },
    plugins::{AppContext, Event, Manifest, McpToolDefinition, Plugin, TauriCommandDefinition},
};

pub(crate) use self::crypto::VaultCipher;

const MANIFEST: Manifest = Manifest {
    name: "vault",
    version: env!("CARGO_PKG_VERSION"),
    description: "Stores secrets and MCP endpoints with encrypted local persistence.",
};

const MIGRATIONS: [MigrationStep; 1] = [MigrationStep {
    name: "0003_create_plugin_vault_tables",
    sql: include_str!("../../../migrations/0003_create_plugin_vault_tables.sql"),
}];

pub fn migrations() -> &'static [MigrationStep] {
    &MIGRATIONS
}

#[derive(Clone)]
pub struct VaultPlugin {
    manifest: Manifest,
    data_store: DataStore,
    cipher: Arc<VaultCipher>,
}

impl VaultPlugin {
    pub fn new(data_store: DataStore) -> Result<Self> {
        let cipher = Arc::new(VaultCipher::from_device()?);
        Ok(Self {
            manifest: MANIFEST,
            data_store,
            cipher,
        })
    }

    #[cfg(test)]
    fn with_cipher(data_store: DataStore, cipher: VaultCipher) -> Self {
        Self {
            manifest: MANIFEST,
            data_store,
            cipher: Arc::new(cipher),
        }
    }

    pub fn list_tokens(&self) -> Result<Vec<StoredVaultToken>> {
        self.data_store.list_vault_tokens()
    }

    pub fn add_token(&self, name: &str, provider: &str, value: &str) -> Result<i64> {
        let encrypted_value = self.cipher.encrypt(value)?;
        self.data_store
            .insert_vault_token(name, provider, &encrypted_value)
    }

    pub fn upsert_token(&self, name: &str, provider: &str, value: &str) -> Result<i64> {
        let encrypted_value = self.cipher.encrypt(value)?;
        if let Some(existing) = self.data_store.get_vault_token_by_provider(provider)? {
            self.data_store
                .update_vault_token(existing.id, name, provider, &encrypted_value)?;
            Ok(existing.id)
        } else {
            self.data_store
                .insert_vault_token(name, provider, &encrypted_value)
        }
    }

    pub fn delete_token(&self, id: i64) -> Result<()> {
        self.data_store.delete_vault_token(id)
    }

    pub fn get_token(&self, id: i64) -> Result<Option<StoredVaultTokenSecret>> {
        let Some(token) = self.data_store.get_vault_token(id)? else {
            return Ok(None);
        };

        let value = self.cipher.decrypt(&token.encrypted_value)?;
        Ok(Some(StoredVaultTokenSecret {
            id: token.id,
            name: token.name,
            provider: token.provider,
            value,
            created_at: token.created_at,
            updated_at: token.updated_at,
        }))
    }

    pub fn get_token_by_provider(&self, provider: &str) -> Result<Option<StoredVaultTokenSecret>> {
        let Some(token) = self.data_store.get_vault_token_by_provider(provider)? else {
            return Ok(None);
        };

        let value = self.cipher.decrypt(&token.encrypted_value)?;
        Ok(Some(StoredVaultTokenSecret {
            id: token.id,
            name: token.name,
            provider: token.provider,
            value,
            created_at: token.created_at,
            updated_at: token.updated_at,
        }))
    }

    pub fn list_mcp_configs(&self) -> Result<Vec<StoredVaultMcpConfig>> {
        self.data_store.list_vault_mcp_configs()
    }

    pub fn update_mcp_config(
        &self,
        id: Option<i64>,
        name: &str,
        transport: &str,
        endpoint: &str,
        enabled: bool,
    ) -> Result<StoredVaultMcpConfig> {
        self.data_store
            .upsert_vault_mcp_config(id, name, transport, endpoint, enabled)
    }
}

impl Plugin for VaultPlugin {
    fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    fn init(&self, _ctx: &AppContext) -> Result<()> {
        let _ = self.data_store.list_vault_mcp_configs()?;
        Ok(())
    }

    fn on_event(&self, _event: &Event) -> Result<()> {
        Ok(())
    }

    fn register_commands(&self) -> Vec<TauriCommandDefinition> {
        vec![
            TauriCommandDefinition {
                name: "vault_list_tokens",
                description: "List stored token metadata without revealing secrets.",
            },
            TauriCommandDefinition {
                name: "vault_add_token",
                description: "Encrypt and store a provider token.",
            },
            TauriCommandDefinition {
                name: "vault_delete_token",
                description: "Delete a stored provider token.",
            },
            TauriCommandDefinition {
                name: "vault_get_token",
                description: "Decrypt and return a stored provider token.",
            },
            TauriCommandDefinition {
                name: "vault_get_token_by_provider",
                description: "Decrypt and return the newest token for a provider.",
            },
            TauriCommandDefinition {
                name: "vault_upsert_token",
                description: "Insert or replace a provider token.",
            },
            TauriCommandDefinition {
                name: "vault_list_mcp",
                description: "List saved MCP endpoint configurations.",
            },
            TauriCommandDefinition {
                name: "vault_update_mcp",
                description: "Create or update an MCP endpoint configuration.",
            },
        ]
    }

    fn mcp_tools(&self) -> Vec<McpToolDefinition> {
        vec![
            McpToolDefinition {
                name: "vault.list_tokens",
                description: "List stored token metadata.",
            },
            McpToolDefinition {
                name: "vault.add_token",
                description: "Store an encrypted provider token.",
            },
            McpToolDefinition {
                name: "vault.delete_token",
                description: "Delete an encrypted provider token.",
            },
            McpToolDefinition {
                name: "vault.get_token",
                description: "Decrypt and return a provider token.",
            },
            McpToolDefinition {
                name: "vault.get_token_by_provider",
                description: "Decrypt and return the newest token for a provider.",
            },
            McpToolDefinition {
                name: "vault.upsert_token",
                description: "Insert or replace a provider token.",
            },
            McpToolDefinition {
                name: "vault.list_mcp",
                description: "List configured MCP endpoints.",
            },
            McpToolDefinition {
                name: "vault.update_mcp",
                description: "Create or update an MCP endpoint configuration.",
            },
        ]
    }

    fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use crate::core::data_store::MigrationPlan;

    use super::*;

    #[test]
    fn vault_plugin_round_trips_tokens_and_mcp_configs() -> Result<()> {
        let data_store = DataStore::in_memory(MigrationPlan::new(migrations()))?;
        let plugin = VaultPlugin::with_cipher(
            data_store.clone(),
            VaultCipher::from_device_identifier("test-device")?,
        );

        let token_id = plugin.add_token("Primary", "openai", "secret-token")?;
        let tokens = plugin.list_tokens()?;
        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].provider, "openai");

        let token = plugin.get_token(token_id)?.expect("token should exist");
        assert_eq!(token.value, "secret-token");
        let token_by_provider = plugin
            .get_token_by_provider("openai")?
            .expect("provider token should exist");
        assert_eq!(token_by_provider.value, "secret-token");

        let same_token_id = plugin.upsert_token("Primary", "openai", "new-secret-token")?;
        assert_eq!(same_token_id, token_id);

        let updated_token = plugin
            .get_token_by_provider("openai")?
            .expect("updated provider token should exist");
        assert_eq!(updated_token.value, "new-secret-token");

        let created =
            plugin.update_mcp_config(None, "Local MCP", "stdio", "npx -y some-mcp", true)?;
        assert_eq!(created.name, "Local MCP");
        assert!(created.enabled);

        let updated = plugin.update_mcp_config(
            Some(created.id),
            "Local MCP",
            "http+sse",
            "http://127.0.0.1:8080/sse",
            false,
        )?;
        assert_eq!(updated.transport, "http+sse");
        assert!(!updated.enabled);

        let mcp_configs = plugin.list_mcp_configs()?;
        assert_eq!(mcp_configs.len(), 1);
        assert_eq!(mcp_configs[0].endpoint, "http://127.0.0.1:8080/sse");

        plugin.delete_token(token_id)?;
        assert!(plugin.get_token(token_id)?.is_none());

        Ok(())
    }
}
