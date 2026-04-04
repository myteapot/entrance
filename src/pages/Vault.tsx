import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import "./Vault.css";
import {
  addVaultToken,
  deleteVaultToken,
  getVaultToken,
  getVaultTokenByProvider,
  GITLAB_UPDATER_NAME,
  GITLAB_UPDATER_PROVIDER,
  listVaultMcpConfigs,
  listVaultTokens,
  updateVaultMcpConfig,
  upsertVaultToken,
  type VaultMcpConfig,
  type VaultToken,
} from "../features/vault/client";

type McpSnippet = {
  client: string;
  path: string;
  content: string;
};

type McpGuide = {
  id: string;
  eyebrow: string;
  title: string;
  description: string;
  launchLabel: string;
  launchValue: string;
  launchHint: string;
  steps: string[];
  snippets: McpSnippet[];
};

const MCP_STDIO_COMMAND = "entrance mcp stdio";
const MCP_HTTP_COMMAND = "entrance mcp http --port 9720 --endpoint /mcp";
const MCP_HTTP_URL = "http://127.0.0.1:9720/mcp";

const renderJson = (value: unknown) => JSON.stringify(value, null, 2);

const mcpSetupGuides: McpGuide[] = [
  {
    id: "stdio",
    eyebrow: "Local child process",
    title: "stdio transport",
    description:
      "Use stdio when Cursor, Claude, or Gemini can launch Entrance directly on the same machine. This is the lowest-friction setup for local development.",
    launchLabel: "Launch command",
    launchValue: MCP_STDIO_COMMAND,
    launchHint:
      "If `entrance` is not on PATH yet, replace it with the full executable path before saving the client config.",
    steps: [
      "Best for a single local machine where the MCP client can spawn Entrance on demand.",
      "Keep the command exactly as shown unless you need an absolute executable path.",
      "After editing the config file, reload or restart your client so the new MCP server is discovered.",
    ],
    snippets: [
      {
        client: "Cursor",
        path: "~/.cursor/mcp.json",
        content: renderJson({
          mcpServers: {
            entrance: {
              type: "stdio",
              command: "entrance",
              args: ["mcp", "stdio"],
            },
          },
        }),
      },
      {
        client: "Claude Code",
        path: "~/.claude.json or project .mcp.json",
        content: renderJson({
          mcpServers: {
            entrance: {
              type: "stdio",
              command: "entrance",
              args: ["mcp", "stdio"],
              env: {},
            },
          },
        }),
      },
      {
        client: "Gemini CLI",
        path: "~/.gemini/settings.json",
        content: renderJson({
          mcpServers: {
            entrance: {
              command: "entrance",
              args: ["mcp", "stdio"],
            },
          },
        }),
      },
    ],
  },
  {
    id: "http",
    eyebrow: "Shared localhost endpoint",
    title: "HTTP transport",
    description:
      "Use HTTP when you want one long-running Entrance MCP endpoint on localhost and multiple clients can attach to it with a URL-based config.",
    launchLabel: "Start server",
    launchValue: MCP_HTTP_COMMAND,
    launchHint:
      "Entrance defaults to port `9720` and endpoint `/mcp`. The resulting local URL is shown below in each client snippet.",
    steps: [
      "Start the MCP HTTP server once in a terminal before opening your AI client.",
      "The default streamable HTTP endpoint is `http://127.0.0.1:9720/mcp`.",
      "If you change the port or endpoint with CLI flags, update the copied JSON snippet to match.",
    ],
    snippets: [
      {
        client: "Cursor",
        path: "~/.cursor/mcp.json",
        content: renderJson({
          mcpServers: {
            entrance: {
              url: MCP_HTTP_URL,
            },
          },
        }),
      },
      {
        client: "Claude Code",
        path: "~/.claude.json or project .mcp.json",
        content: renderJson({
          mcpServers: {
            entrance: {
              type: "http",
              url: MCP_HTTP_URL,
            },
          },
        }),
      },
      {
        client: "Gemini CLI",
        path: "~/.gemini/settings.json",
        content: renderJson({
          mcpServers: {
            entrance: {
              httpUrl: MCP_HTTP_URL,
            },
          },
        }),
      },
    ],
  },
];

export default function Vault() {
  const [tokens, setTokens] = createSignal<VaultToken[]>([]);
  const [mcpConfigs, setMcpConfigs] = createSignal<VaultMcpConfig[]>([]);
  const [tokenSecrets, setTokenSecrets] = createSignal<Record<number, string>>({});
  const [visibleTokenIds, setVisibleTokenIds] = createSignal<Set<number>>(new Set());
  const [isLoading, setIsLoading] = createSignal(true);
  const [isSavingGitlabToken, setIsSavingGitlabToken] = createSignal(false);
  const [feedbackTone, setFeedbackTone] = createSignal<"success" | "error" | null>(null);
  const [feedbackMessage, setFeedbackMessage] = createSignal<string | null>(null);
  const [showTokenModal, setShowTokenModal] = createSignal(false);
  const [showMcpModal, setShowMcpModal] = createSignal(false);

  const [newTokenName, setNewTokenName] = createSignal("");
  const [newTokenProvider, setNewTokenProvider] = createSignal("");
  const [newTokenValue, setNewTokenValue] = createSignal("");
  const [gitlabTokenValue, setGitlabTokenValue] = createSignal("");
  const [gitlabPreviewValue, setGitlabPreviewValue] = createSignal<string | null>(null);
  const [showGitlabPreview, setShowGitlabPreview] = createSignal(false);
  const [showCopyToast, setShowCopyToast] = createSignal(false);

  const [newMcpName, setNewMcpName] = createSignal("");
  const [newMcpEndpoint, setNewMcpEndpoint] = createSignal("");
  const [newMcpTransport, setNewMcpTransport] = createSignal("stdio");
  let copyToastTimer: number | undefined;

  const setFeedback = (message: string, tone: "success" | "error") => {
    setFeedbackTone(tone);
    setFeedbackMessage(message);
  };

  const formatTimestamp = (value: string) => new Date(value).toLocaleString();

  const maskTokenValue = (value: string) => {
    if (!value) {
      return "Not configured";
    }

    if (value.length <= 8) {
      return "*".repeat(Math.max(8, value.length));
    }

    return `${value.slice(0, 4)}${"*".repeat(Math.max(4, value.length - 8))}${value.slice(-4)}`;
  };

  const gitlabToken = () =>
    tokens().find((token) => token.provider === GITLAB_UPDATER_PROVIDER) ?? null;

  const apiTokens = () =>
    tokens().filter((token) => token.provider !== GITLAB_UPDATER_PROVIDER);

  const refreshVaultData = async () => {
    setIsLoading(true);
    try {
      const [nextTokens, nextMcpConfigs] = await Promise.all([
        listVaultTokens(),
        listVaultMcpConfigs(),
      ]);
      setTokens(nextTokens);
      setMcpConfigs(nextMcpConfigs);
    } catch (error) {
      console.error("Failed to load Vault data", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to load Vault data.",
        "error",
      );
    } finally {
      setIsLoading(false);
    }
  };

  onMount(() => {
    void refreshVaultData();
  });

  onCleanup(() => {
    if (copyToastTimer !== undefined) {
      window.clearTimeout(copyToastTimer);
    }
  });

  const flashCopyToast = () => {
    setShowCopyToast(true);
    if (copyToastTimer !== undefined) {
      window.clearTimeout(copyToastTimer);
    }
    copyToastTimer = window.setTimeout(() => {
      setShowCopyToast(false);
    }, 1600);
  };

  const copyToClipboard = async (value: string) => {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(value);
      } else {
        const input = document.createElement("textarea");
        input.value = value;
        input.setAttribute("readonly", "");
        input.style.position = "absolute";
        input.style.left = "-9999px";
        document.body.append(input);
        input.select();
        document.execCommand("copy");
        input.remove();
      }

      flashCopyToast();
    } catch (error) {
      console.error("Failed to copy MCP configuration", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to copy to the clipboard.",
        "error",
      );
    }
  };

  const toggleTokenVisibility = async (id: number) => {
    const nextVisibleIds = new Set(visibleTokenIds());
    if (nextVisibleIds.has(id)) {
      nextVisibleIds.delete(id);
      setVisibleTokenIds(nextVisibleIds);
      return;
    }

    const cachedSecret = tokenSecrets()[id];
    if (!cachedSecret) {
      try {
        const token = await getVaultToken(id);
        if (token?.value) {
          setTokenSecrets((current) => ({
            ...current,
            [id]: token.value,
          }));
        }
      } catch (error) {
        console.error("Failed to reveal token", error);
        setFeedback(
          error instanceof Error ? error.message : "Failed to reveal token.",
          "error",
        );
        return;
      }
    }

    nextVisibleIds.add(id);
    setVisibleTokenIds(nextVisibleIds);
  };

  const handleSaveGitlabToken = async () => {
    const value = gitlabTokenValue().trim();
    if (!value) {
      setFeedback("Paste the GitLab Bot token before saving.", "error");
      return;
    }

    setIsSavingGitlabToken(true);
    try {
      await upsertVaultToken(
        GITLAB_UPDATER_NAME,
        GITLAB_UPDATER_PROVIDER,
        value,
      );
      setGitlabTokenValue("");
      setGitlabPreviewValue(null);
      setShowGitlabPreview(false);
      await refreshVaultData();
      setFeedback(
        "GitLab Bot token saved. Future update checks will send PRIVATE-TOKEN.",
        "success",
      );
    } catch (error) {
      console.error("Failed to save GitLab updater token", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to save GitLab Bot token.",
        "error",
      );
    } finally {
      setIsSavingGitlabToken(false);
    }
  };

  const handleRevealGitlabToken = async () => {
    if (showGitlabPreview()) {
      setShowGitlabPreview(false);
      return;
    }

    try {
      if (!gitlabPreviewValue()) {
        const token = await getVaultTokenByProvider(GITLAB_UPDATER_PROVIDER);
        setGitlabPreviewValue(token?.value ?? null);
      }
      setShowGitlabPreview(true);
    } catch (error) {
      console.error("Failed to reveal GitLab updater token", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to reveal GitLab Bot token.",
        "error",
      );
    }
  };

  const handleDeleteToken = async (id: number) => {
    try {
      await deleteVaultToken(id);
      setVisibleTokenIds((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
      setTokenSecrets((current) => {
        const next = { ...current };
        delete next[id];
        return next;
      });

      if (gitlabToken()?.id === id) {
        setGitlabPreviewValue(null);
        setShowGitlabPreview(false);
      }

      await refreshVaultData();
      setFeedback("Token deleted from Vault.", "success");
    } catch (error) {
      console.error("Failed to delete token", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to delete token.",
        "error",
      );
    }
  };

  const handleAddToken = async () => {
    const name = newTokenName().trim();
    const provider = newTokenProvider().trim();
    const value = newTokenValue().trim();
    if (!name || !provider || !value) {
      setFeedback("Name, provider, and token value are required.", "error");
      return;
    }

    try {
      await addVaultToken(name, provider, value);
      setShowTokenModal(false);
      setNewTokenName("");
      setNewTokenProvider("");
      setNewTokenValue("");
      await refreshVaultData();
      setFeedback("Token added to Vault.", "success");
    } catch (error) {
      console.error("Failed to add token", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to add token.",
        "error",
      );
    }
  };

  const handleAddMcp = async () => {
    const name = newMcpName().trim();
    const endpoint = newMcpEndpoint().trim();
    if (!name || !endpoint) {
      setFeedback("MCP name and endpoint are required.", "error");
      return;
    }

    try {
      await updateVaultMcpConfig(
        null,
        name,
        newMcpTransport(),
        endpoint,
        true,
      );
      setShowMcpModal(false);
      setNewMcpName("");
      setNewMcpEndpoint("");
      setNewMcpTransport("stdio");
      await refreshVaultData();
      setFeedback("MCP configuration saved.", "success");
    } catch (error) {
      console.error("Failed to save MCP configuration", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to save MCP configuration.",
        "error",
      );
    }
  };

  const handleToggleMcp = async (config: VaultMcpConfig) => {
    try {
      await updateVaultMcpConfig(
        config.id,
        config.name,
        config.transport,
        config.endpoint,
        !config.enabled,
      );
      await refreshVaultData();
    } catch (error) {
      console.error("Failed to update MCP configuration", error);
      setFeedback(
        error instanceof Error ? error.message : "Failed to update MCP configuration.",
        "error",
      );
    }
  };

  return (
    <div class="vault-page">
      <div class="vault-header">
        <h1 class="vault-title">Vault</h1>
        <p class="vault-subtitle">Manage encrypted tokens and configure Model Context Protocol (MCP) servers safely.</p>
      </div>

      <Show when={showCopyToast()}>
        <div class="vault-toast" role="status" aria-live="polite">
          Copied!
        </div>
      </Show>

      <Show when={feedbackMessage()}>
        {(message) => (
          <div class={`vault-callout vault-callout--${feedbackTone() ?? "success"}`}>
            {message()}
          </div>
        )}
      </Show>

      <Show when={isLoading()}>
        <div class="vault-loading">Loading encrypted Vault data...</div>
      </Show>

      <section class="vault-section vault-section--guide">
        <div class="vault-section-header vault-section-header--stack">
          <div>
            <h2 class="vault-section-title">Connect Entrance as an MCP server</h2>
            <p class="vault-section-copy">
              Entrance already exposes MCP over both local stdio and localhost HTTP. Copy the
              snippet that matches your AI client, paste it into that client&apos;s config, and
              reload the client.
            </p>
          </div>
          <span class="vault-status-pill is-configured">MCP ready</span>
        </div>

        <div class="vault-mcp-guides">
          <For each={mcpSetupGuides}>
            {(guide) => (
              <article class="vault-mcp-guide">
                <div class="vault-mcp-guide__header">
                  <div>
                    <span class="vault-label">{guide.eyebrow}</span>
                    <h3 class="vault-mcp-guide__title">{guide.title}</h3>
                  </div>
                  <button class="btn btn-primary" onClick={() => void copyToClipboard(guide.launchValue)}>
                    Copy command
                  </button>
                </div>

                <p class="vault-section-copy vault-section-copy--tight">{guide.description}</p>

                <div class="vault-mcp-command">
                  <div>
                    <span class="vault-label">{guide.launchLabel}</span>
                    <code class="vault-command-code">{guide.launchValue}</code>
                  </div>
                </div>

                <p class="vault-form-note">{guide.launchHint}</p>

                <ul class="vault-mcp-steps">
                  <For each={guide.steps}>
                    {(step) => <li>{step}</li>}
                  </For>
                </ul>

                <div class="vault-mcp-snippets">
                  <For each={guide.snippets}>
                    {(snippet) => (
                      <section class="vault-snippet-card">
                        <div class="vault-snippet-card__header">
                          <div>
                            <span class="vault-label">{snippet.client}</span>
                            <strong>{snippet.path}</strong>
                          </div>
                          <button class="btn-icon" onClick={() => void copyToClipboard(snippet.content)}>
                            Copy JSON
                          </button>
                        </div>
                        <pre class="vault-snippet-card__body">
                          <code>{snippet.content}</code>
                        </pre>
                      </section>
                    )}
                  </For>
                </div>
              </article>
            )}
          </For>
        </div>
      </section>

      <section class="vault-section vault-section--feature">
        <div class="vault-section-header vault-section-header--stack">
          <div>
            <h2 class="vault-section-title">Entrance Updater Token</h2>
            <p class="vault-section-copy">
              This GitLab Bot token is used only by Entrance itself when it checks for updates and downloads private release artifacts from GitLab.
            </p>
          </div>
          <span class={`vault-status-pill ${gitlabToken() ? "is-configured" : "is-missing"}`}>
            {gitlabToken() ? "Configured" : "Not configured"}
          </span>
        </div>

        <div class="vault-updater-card">
          <div class="vault-updater-meta">
            <div>
              <span class="vault-label">Display name</span>
              <strong>{GITLAB_UPDATER_NAME}</strong>
            </div>
            <div>
              <span class="vault-label">Provider key</span>
              <strong>{GITLAB_UPDATER_PROVIDER}</strong>
            </div>
            <Show when={gitlabToken()}>
              {(token) => (
                <div>
                  <span class="vault-label">Last updated</span>
                  <strong>{formatTimestamp(token().updated_at)}</strong>
                </div>
              )}
            </Show>
          </div>

          <div class="form-group">
            <label class="form-label">GitLab Access Token</label>
            <input
              class="form-input"
              type="password"
              value={gitlabTokenValue()}
              onInput={(event) => setGitlabTokenValue(event.currentTarget.value)}
              placeholder={
                gitlabToken()
                  ? "Paste a new Bot token to replace the stored value"
                  : "Paste the Bot token used for updater requests"
              }
            />
            <p class="form-hint">
              Entrance sends this as the <code>PRIVATE-TOKEN</code> header for updater metadata and package downloads.
            </p>
          </div>

          <Show when={gitlabToken()}>
            <div class="vault-secret-preview">
              <span class="vault-label">Stored value</span>
              <code class="password-text">
                {showGitlabPreview()
                  ? gitlabPreviewValue() ?? "Unavailable"
                  : maskTokenValue(gitlabPreviewValue() ?? "configured")}
              </code>
            </div>
          </Show>

          <div class="modal-actions vault-actions">
            <Show when={gitlabToken()}>
              <button class="btn" onClick={() => void handleRevealGitlabToken()}>
                {showGitlabPreview() ? "Hide Token" : "Reveal Token"}
              </button>
            </Show>
            <Show when={gitlabToken()}>
              {(token) => (
                <button class="btn" onClick={() => void handleDeleteToken(token().id)}>
                  Clear Token
                </button>
              )}
            </Show>
            <button
              class="btn btn-primary"
              disabled={isSavingGitlabToken()}
              onClick={() => void handleSaveGitlabToken()}
            >
              {isSavingGitlabToken() ? "Saving..." : gitlabToken() ? "Replace Token" : "Save Token"}
            </button>
          </div>
        </div>
      </section>

      <section class="vault-section">
        <div class="vault-section-header">
          <h2 class="vault-section-title">API Tokens</h2>
          <button class="btn btn-primary" onClick={() => setShowTokenModal(true)}>+ New Token</button>
        </div>
        <table class="vault-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Provider</th>
              <th>Updated</th>
              <th>Token Value</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <For each={apiTokens()}>
              {(token) => (
                <tr>
                  <td>{token.name}</td>
                  <td>{token.provider}</td>
                  <td>{formatTimestamp(token.updated_at)}</td>
                  <td>
                    <div class="password-display">
                      <code class="password-text">
                        {visibleTokenIds().has(token.id)
                          ? tokenSecrets()[token.id] ?? "Unavailable"
                          : "************"}
                      </code>
                      <button
                        class="btn-icon"
                        onClick={() => void toggleTokenVisibility(token.id)}
                        title="Toggle Visibility"
                      >
                        {visibleTokenIds().has(token.id) ? "Hide" : "Show"}
                      </button>
                    </div>
                  </td>
                  <td>
                    <button class="btn-icon" onClick={() => void handleDeleteToken(token.id)}>Delete</button>
                  </td>
                </tr>
              )}
            </For>
            <Show when={apiTokens().length === 0}>
              <tr>
                <td colSpan={5} style={{ "text-align": "center", color: "var(--text-tertiary)" }}>No API tokens configured.</td>
              </tr>
            </Show>
          </tbody>
        </table>
      </section>

      <section class="vault-section">
        <div class="vault-section-header">
          <h2 class="vault-section-title">MCP Configurations</h2>
          <button class="btn btn-primary" onClick={() => setShowMcpModal(true)}>+ Add MCP</button>
        </div>
        <table class="vault-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Transport</th>
              <th>Endpoint / Command</th>
              <th>Status</th>
              <th>Updated</th>
            </tr>
          </thead>
          <tbody>
            <For each={mcpConfigs()}>
              {(mcp) => (
                <tr>
                  <td>{mcp.name}</td>
                  <td>
                    <code class="password-text">{mcp.transport}</code>
                  </td>
                  <td>{mcp.endpoint}</td>
                  <td>
                    <label class="toggle-switch">
                      <input type="checkbox" checked={mcp.enabled} onChange={() => void handleToggleMcp(mcp)} />
                      <span class="slider"></span>
                    </label>
                  </td>
                  <td>{formatTimestamp(mcp.updated_at)}</td>
                </tr>
              )}
            </For>
            <Show when={mcpConfigs().length === 0}>
              <tr>
                <td colSpan={5} style={{ "text-align": "center", color: "var(--text-tertiary)" }}>No MCP servers configured.</td>
              </tr>
            </Show>
          </tbody>
        </table>
      </section>

      <Show when={showTokenModal()}>
        <div class="modal-backdrop">
          <div class="modal">
            <h2 class="vault-title" style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>Add New Token</h2>
            <div class="form-group">
              <label class="form-label">Name</label>
              <input class="form-input" type="text" value={newTokenName()} onInput={(event) => setNewTokenName(event.currentTarget.value)} placeholder="e.g. Primary OpenAI Key" />
            </div>
            <div class="form-group">
              <label class="form-label">Provider</label>
              <input class="form-input" type="text" value={newTokenProvider()} onInput={(event) => setNewTokenProvider(event.currentTarget.value)} placeholder="e.g. openai, anthropic, gemini" />
              <p class="form-hint">Forge required tokens match this provider key.</p>
            </div>
            <div class="form-group">
              <label class="form-label">Token Value</label>
              <input class="form-input" type="password" value={newTokenValue()} onInput={(event) => setNewTokenValue(event.currentTarget.value)} placeholder="Paste token here..." />
            </div>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowTokenModal(false)}>Cancel</button>
              <button class="btn btn-primary" onClick={() => void handleAddToken()}>Save Token</button>
            </div>
          </div>
        </div>
      </Show>

      <Show when={showMcpModal()}>
        <div class="modal-backdrop">
          <div class="modal">
            <h2 class="vault-title" style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>Add MCP Configuration</h2>
            <div class="form-group">
              <label class="form-label">Name</label>
              <input class="form-input" type="text" value={newMcpName()} onInput={(event) => setNewMcpName(event.currentTarget.value)} placeholder="e.g. Local Database" />
            </div>
            <div class="form-group">
              <label class="form-label">Transport</label>
              <select class="form-select" value={newMcpTransport()} onChange={(event) => setNewMcpTransport(event.currentTarget.value)}>
                <option value="stdio">stdio</option>
                <option value="http+sse">http+sse</option>
              </select>
            </div>
            <div class="form-group">
              <label class="form-label">Endpoint / Command</label>
              <input class="form-input" type="text" value={newMcpEndpoint()} onInput={(event) => setNewMcpEndpoint(event.currentTarget.value)} placeholder={newMcpTransport() === "stdio" ? "e.g. npx -y @modelcontextprotocol/server-sqlite" : "e.g. http://localhost:8080/sse"} />
              <p class="form-hint">For stdio, place the full command and arguments in the endpoint field.</p>
            </div>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowMcpModal(false)}>Cancel</button>
              <button class="btn btn-primary" onClick={() => void handleAddMcp()}>Save Configuration</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
