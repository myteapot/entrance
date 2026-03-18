import "./App.css";

const workspaceLayers = [
  {
    label: "Frontend",
    path: "src/",
    detail: "SolidJS application shell and future dashboard surface.",
  },
  {
    label: "Native",
    path: "src-tauri/",
    detail: "Tauri 2 runtime, Rust host process, and bundle configuration.",
  },
  {
    label: "Specs",
    path: "specs/",
    detail: "Living architecture notes that drive the next implementation slices.",
  },
  {
    label: "Config",
    path: "entrance.toml",
    detail: "Placeholder runtime configuration that will expand with later milestones.",
  },
] as const;

function App() {
  return (
    <main class="app-shell">
      <section class="hero">
        <p class="eyebrow">Desktop Foundation</p>
        <h1>Entrance</h1>
        <p class="summary">
          Tauri 2.0 and SolidJS are wired up and ready for the backend slices
          described in <code>specs/backend.md</code>.
        </p>
      </section>

      <section class="layer-grid" aria-label="Workspace structure overview">
        {workspaceLayers.map((layer) => (
          <article class="layer-card">
            <div class="layer-header">
              <span>{layer.label}</span>
              <code>{layer.path}</code>
            </div>
            <p>{layer.detail}</p>
          </article>
        ))}
      </section>
    </main>
  );
}

export default App;
