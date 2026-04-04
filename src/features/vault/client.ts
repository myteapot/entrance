import { invoke } from "@tauri-apps/api/core";

export const GITLAB_UPDATER_PROVIDER = "gitlab-updater";
export const GITLAB_UPDATER_NAME = "Entrance GitLab Bot";

export type VaultToken = {
  id: number;
  name: string;
  provider: string;
  created_at: string;
  updated_at: string;
};

export type VaultTokenSecret = VaultToken & {
  value: string;
};

export type VaultMcpConfig = {
  id: number;
  name: string;
  transport: string;
  endpoint: string;
  enabled: boolean;
  created_at: string;
  updated_at: string;
};

export const listVaultTokens = () => invoke<VaultToken[]>("vault_list_tokens");

export const addVaultToken = (name: string, provider: string, value: string) =>
  invoke<number>("vault_add_token", { name, provider, value });

export const upsertVaultToken = (name: string, provider: string, value: string) =>
  invoke<number>("vault_upsert_token", { name, provider, value });

export const deleteVaultToken = (id: number) => invoke<void>("vault_delete_token", { id });

export const getVaultToken = (id: number) =>
  invoke<VaultTokenSecret | null>("vault_get_token", { id });

export const getVaultTokenByProvider = (provider: string) =>
  invoke<VaultTokenSecret | null>("vault_get_token_by_provider", { provider });

export const listVaultMcpConfigs = () =>
  invoke<VaultMcpConfig[]>("vault_list_mcp");

export const updateVaultMcpConfig = (
  id: number | null,
  name: string,
  transport: string,
  endpoint: string,
  enabled: boolean,
) =>
  invoke<VaultMcpConfig>("vault_update_mcp", {
    id,
    name,
    transport,
    endpoint,
    enabled,
  });
