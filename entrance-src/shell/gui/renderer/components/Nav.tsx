type NavKey = "status" | "drawer" | "hive" | "launcher";

type NavProps = {
  current: NavKey;
  onSelect: (value: NavKey) => void;
};

const items: Array<{ key: NavKey; label: string; detail: string }> = [
  { key: "status", label: "Status", detail: "Kernel snapshot" },
  { key: "drawer", label: "Drawer", detail: "File and note intake" },
  { key: "hive", label: "Hive", detail: "Dispatch surface" },
  { key: "launcher", label: "Launcher", detail: "Search and launch" },
];

export default function Nav(props: NavProps) {
  return (
    <aside class="nav-shell">
      <div class="nav-brand">
        <p class="nav-kicker">Entrance V2</p>
        <h1>Microkernel</h1>
        <p class="nav-copy">Single binary runtime with isolated plugins and an Electron shell.</p>
      </div>

      <nav class="nav-list" aria-label="Primary">
        {items.map((item) => (
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
