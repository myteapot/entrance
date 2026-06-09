type NavKey = "status" | "drawer" | "hive" | "panel" | "reviews" | "loops" | "launcher";

type NavProps = {
  current: NavKey;
  onSelect: (value: NavKey) => void;
};

const items: Array<{ key: NavKey; label: string; detail: string; section?: "main" | "tools" }> = [
  { key: "panel", label: "Issues", detail: "Agent board", section: "main" },
  { key: "reviews", label: "Reviews", detail: "Human gates", section: "main" },
  { key: "loops", label: "Loops", detail: "Runtime contracts", section: "main" },
  { key: "status", label: "Status", detail: "Kernel health", section: "tools" },
  { key: "launcher", label: "Launcher", detail: "Search and launch", section: "tools" },
  { key: "drawer", label: "Drawer", detail: "File and note intake", section: "tools" },
];

export default function Nav(props: NavProps) {
  return (
    <aside class="nav-shell">
      <div class="nav-brand">
        <p class="nav-kicker">Entrance</p>
        <h1>Workbench</h1>
        <p class="nav-copy">Local agent issues</p>
      </div>

      <nav class="nav-list" aria-label="Primary">
        {items
          .filter((item) => item.section === "main")
          .map((item) => (
            <button
              type="button"
              class={`nav-item ${props.current === item.key ? "is-active" : ""}`}
              onClick={() => props.onSelect(item.key)}
            >
              <span>{item.label}</span>
              <small>{item.detail}</small>
            </button>
          ))}
        <div class="nav-section-label">Tools</div>
        {items
          .filter((item) => item.section === "tools")
          .map((item) => (
            <button
              type="button"
              class={`nav-item ${props.current === item.key ? "is-active" : ""}`}
              onClick={() => props.onSelect(item.key)}
            >
              <span>{item.label}</span>
              <small>{item.detail}</small>
            </button>
          ))}
      </nav>
    </aside>
  );
}
