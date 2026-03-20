const widgetPlaceholders = [
  {
    title: "Launch queue",
    caption: "Pending entry points",
    detail: "Launcher hooks will surface recent commands, pinned flows, and execution status here.",
  },
  {
    title: "Forge pulse",
    caption: "Draft workspace widgets",
    detail: "Forge modules can register cards into this strip once plugin discovery is wired in.",
  },
  {
    title: "Connector stream",
    caption: "Comm replacement slot",
    detail: "The renamed connector route keeps a dedicated space for sync health and external bridges.",
  },
] as const;

const Dashboard = () => {
  return (
    <section class="page page--dashboard">
      <header class="page__hero">
        <p class="page__eyebrow">Dashboard</p>
        <h2>Welcome to Entrance</h2>
        <p class="page__summary">
          The desktop shell is now split into a persistent sidebar and a routed main panel, ready for plugin pages and
          Tauri IPC wiring.
        </p>
      </header>

      <section class="dashboard-grid" aria-label="Dashboard widget placeholders">
        {widgetPlaceholders.map((widget) => (
          <article class="dashboard-card">
            <p class="dashboard-card__caption">{widget.caption}</p>
            <h3>{widget.title}</h3>
            <p>{widget.detail}</p>
          </article>
        ))}
      </section>

      <section class="dashboard-panel">
        <div>
          <p class="dashboard-panel__eyebrow">Next integration</p>
          <h3>Plugin widget host placeholder</h3>
        </div>
        <p>
          This surface intentionally stays empty for now. Later slices can hydrate it from the Rust-side plugin manager
          without revisiting the overall layout contract.
        </p>
      </section>
    </section>
  );
};

export default Dashboard;
