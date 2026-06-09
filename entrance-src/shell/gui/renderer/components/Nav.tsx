import type { View } from "../lib/hive";

type NavKey = "panel" | "reviews" | "diagnostics";

type NavProps = {
  current: View;
  onSelect: (value: View) => void;
};

const diagnosticViews = new Set<View>(["diagnostics", "status", "drawer", "hive", "loops", "launcher"]);

const items: Array<{ key: NavKey; label: string; detail: string }> = [
  { key: "panel", label: "Issues", detail: "Agent board" },
  { key: "reviews", label: "Reviews", detail: "Decision inbox" },
  { key: "diagnostics", label: "Diagnostics", detail: "Tools and runtime" },
];

const isActive = (current: View, key: NavKey) =>
  key === "diagnostics" ? diagnosticViews.has(current) : current === key;

export default function Nav(props: NavProps) {
  return (
    <aside class="nav-shell">
      <div class="nav-brand">
        <p class="nav-kicker">Entrance</p>
        <h1>Workbench</h1>
        <p class="nav-copy">Local agent issues</p>
      </div>

      <nav class="nav-list" aria-label="Primary">
        {items.map((item) => (
          <button
            type="button"
            class={`nav-item ${isActive(props.current, item.key) ? "is-active" : ""}`}
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
