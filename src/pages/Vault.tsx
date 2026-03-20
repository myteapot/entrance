import { createSignal, For, Show } from "solid-js";
import { createStore } from "solid-js/store";
import "./Vault.css";

// --- Types ---
type ApiToken = {
  id: string;
  name: string;
  provider: string;
  value: string;
};

type McpConfig = {
  id: string;
  name: string;
  endpoint: string;
  enabled: boolean;
  transport?: string;
  args?: string;
};

// --- Mock Data ---
const initialTokens: ApiToken[] = [
  { id: "1", name: "Default Gemini", provider: "Google", value: "AIzaSyB................" },
  { id: "2", name: "Anthropic Claude", provider: "Anthropic", value: "sk-ant-................" },
];

const initialMcpConfigs: McpConfig[] = [
  { id: "1", name: "Local Linear MCP", endpoint: "linear-mcp-server", transport: "stdio", enabled: true },
  { id: "2", name: "Search Utility", endpoint: "http://localhost:8080/mcp", transport: "http", enabled: false },
];

export default function Vault() {
  const [tokens, setTokens] = createStore<ApiToken[]>(initialTokens);
  const [mcpConfigs, setMcpConfigs] = createStore<McpConfig[]>(initialMcpConfigs);

  // UI States
  const [showTokenModal, setShowTokenModal] = createSignal(false);
  const [showMcpModal, setShowMcpModal] = createSignal(false);

  // Add Token Modal State
  const [newTokenName, setNewTokenName] = createSignal("");
  const [newTokenProvider, setNewTokenProvider] = createSignal("");
  const [newTokenValue, setNewTokenValue] = createSignal("");

  // Add MCP Modal State
  const [newMcpName, setNewMcpName] = createSignal("");
  const [newMcpEndpoint, setNewMcpEndpoint] = createSignal("");
  const [newMcpTransport, setNewMcpTransport] = createSignal("stdio");
  const [newMcpArgs, setNewMcpArgs] = createSignal("");

  // Visible Passwords Set
  const [visiblePasswords, setVisiblePasswords] = createSignal<Set<string>>(new Set());

  const togglePasswordVisibility = (id: string) => {
    const newSet = new Set(visiblePasswords());
    if (newSet.has(id)) {
      newSet.delete(id);
    } else {
      newSet.add(id);
    }
    setVisiblePasswords(newSet);
  };

  const handleAddToken = () => {
    if (newTokenName() && newTokenProvider() && newTokenValue()) {
      setTokens([...tokens, {
        id: Date.now().toString(),
        name: newTokenName(),
        provider: newTokenProvider(),
        value: newTokenValue(),
      }]);
      setShowTokenModal(false);
      setNewTokenName("");
      setNewTokenProvider("");
      setNewTokenValue("");
    }
  };

  const handleDeleteToken = (id: string) => {
    setTokens(tokens.filter(t => t.id !== id));
  };

  const handleAddMcp = () => {
    if (newMcpName() && newMcpEndpoint()) {
      setMcpConfigs([...mcpConfigs, {
        id: Date.now().toString(),
        name: newMcpName(),
        endpoint: newMcpEndpoint(),
        transport: newMcpTransport(),
        args: newMcpArgs(),
        enabled: true,
      }]);
      setShowMcpModal(false);
      setNewMcpName("");
      setNewMcpEndpoint("");
      setNewMcpTransport("stdio");
      setNewMcpArgs("");
    }
  };

  const handleToggleMcp = (id: string) => {
    setMcpConfigs(c => c.id === id, "enabled", e => !e);
  };

  const handleDeleteMcp = (id: string) => {
    setMcpConfigs(mcpConfigs.filter(m => m.id !== id));
  };

  return (
    <div class="vault-page">
      <div class="vault-header">
        <h1 class="vault-title">Vault</h1>
        <p class="vault-subtitle">Manage API Tokens and configure Model Context Protocol (MCP) servers safely.</p>
      </div>

      {/* --- Tokens Section --- */}
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
              <th>Token Value</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <For each={tokens}>
              {(token) => (
                <tr>
                  <td>{token.name}</td>
                  <td>{token.provider}</td>
                  <td>
                    <div class="password-display">
                      <span class="password-text">
                        {visiblePasswords().has(token.id) ? token.value : "••••••••••••••••••••"}
                      </span>
                      <button class="btn-icon" onClick={() => togglePasswordVisibility(token.id)} title="Toggle Visibility">
                        {visiblePasswords().has(token.id) ? "Hide" : "Show"}
                      </button>
                    </div>
                  </td>
                  <td>
                    <button class="btn-icon" onClick={() => handleDeleteToken(token.id)}>Delete</button>
                  </td>
                </tr>
              )}
            </For>
            <Show when={tokens.length === 0}>
              <tr>
                <td colspan="4" style={{ "text-align": "center", color: "var(--text-tertiary)" }}>No tokens configured.</td>
              </tr>
            </Show>
          </tbody>
        </table>
      </section>

      {/* --- MCP Configs Section --- */}
      <section class="vault-section">
        <div class="vault-section-header">
          <h2 class="vault-section-title">MCP Configurations</h2>
          <button class="btn btn-primary" onClick={() => setShowMcpModal(true)}>+ Add MCP</button>
        </div>
        <table class="vault-table">
          <thead>
            <tr>
              <th>Name</th>
              <th>Endpoint / Transport</th>
              <th>Status</th>
              <th>Actions</th>
            </tr>
          </thead>
          <tbody>
            <For each={mcpConfigs}>
              {(mcp) => (
                <tr>
                  <td>{mcp.name}</td>
                  <td>
                    <div style={{ "margin-bottom": "4px" }}>{mcp.endpoint}</div>
                    <span class="password-text">{mcp.transport}</span>
                  </td>
                  <td>
                    <label class="toggle-switch">
                      <input type="checkbox" checked={mcp.enabled} onChange={() => handleToggleMcp(mcp.id)} />
                      <span class="slider"></span>
                    </label>
                  </td>
                  <td>
                    <button class="btn-icon" onClick={() => handleDeleteMcp(mcp.id)}>Delete</button>
                  </td>
                </tr>
              )}
            </For>
            <Show when={mcpConfigs.length === 0}>
              <tr>
                <td colspan="4" style={{ "text-align": "center", color: "var(--text-tertiary)" }}>No MCP servers configured.</td>
              </tr>
            </Show>
          </tbody>
        </table>
      </section>

      {/* Token Modal */}
      <Show when={showTokenModal()}>
        <div class="modal-backdrop">
          <div class="modal">
            <h2 class="vault-title" style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>Add New Token</h2>
            <div class="form-group">
              <label class="form-label">Name</label>
              <input class="form-input" type="text" value={newTokenName()} onInput={(e) => setNewTokenName(e.currentTarget.value)} placeholder="e.g. My Prod Key" />
            </div>
            <div class="form-group">
              <label class="form-label">Provider</label>
              <select class="form-select" value={newTokenProvider()} onChange={(e) => setNewTokenProvider(e.currentTarget.value)}>
                <option value="">Select a provider...</option>
                <option value="Google">Google (Gemini)</option>
                <option value="Anthropic">Anthropic (Claude)</option>
                <option value="OpenAI">OpenAI</option>
                <option value="Custom">Custom</option>
              </select>
            </div>
            <div class="form-group">
              <label class="form-label">Token Value</label>
              <input class="form-input" type="password" value={newTokenValue()} onInput={(e) => setNewTokenValue(e.currentTarget.value)} placeholder="Paste token here..." />
            </div>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowTokenModal(false)}>Cancel</button>
              <button class="btn btn-primary" onClick={handleAddToken}>Save Token</button>
            </div>
          </div>
        </div>
      </Show>

      {/* MCP Modal */}
      <Show when={showMcpModal()}>
        <div class="modal-backdrop">
          <div class="modal">
            <h2 class="vault-title" style={{ "margin-bottom": "var(--space-4)", "font-size": "var(--text-xl)" }}>Add MCP Configuration</h2>
            <div class="form-group">
              <label class="form-label">Name</label>
              <input class="form-input" type="text" value={newMcpName()} onInput={(e) => setNewMcpName(e.currentTarget.value)} placeholder="e.g. Local Database" />
            </div>
            <div class="form-group">
              <label class="form-label">Transport</label>
              <select class="form-select" value={newMcpTransport()} onChange={(e) => setNewMcpTransport(e.currentTarget.value)}>
                <option value="stdio">stdio</option>
                <option value="sse">sse (HTTP)</option>
              </select>
            </div>
            <div class="form-group">
              <label class="form-label">Endpoint / Command</label>
              <input class="form-input" type="text" value={newMcpEndpoint()} onInput={(e) => setNewMcpEndpoint(e.currentTarget.value)} placeholder={newMcpTransport() === 'stdio' ? "e.g. npx" : "e.g. http://localhost:8080/mcp"} />
            </div>
            <Show when={newMcpTransport() === 'stdio'}>
              <div class="form-group">
                <label class="form-label">Arguments (optional)</label>
                <input class="form-input" type="text" value={newMcpArgs()} onInput={(e) => setNewMcpArgs(e.currentTarget.value)} placeholder="e.g. -y @modelcontextprotocol/server-sqlite" />
              </div>
            </Show>
            <div class="modal-actions">
              <button class="btn" onClick={() => setShowMcpModal(false)}>Cancel</button>
              <button class="btn btn-primary" onClick={handleAddMcp}>Save Configuration</button>
            </div>
          </div>
        </div>
      </Show>
    </div>
  );
}
