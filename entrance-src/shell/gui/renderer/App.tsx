import { Match, Switch, createResource, createSignal } from "solid-js";
import Nav from "./components/Nav";
import { bridge } from "./lib/bridge";

type View = "status" | "drawer" | "hive" | "launcher";

type AppStatus = {
  app_root: string;
  db_path: string;
  drawer_entries: number;
  hive_runs: number;
  launcher_entries: number;
  generated_at: string;
};

type DrawerSummary = {
  mode: string;
  root: string;
  items: number;
};

type DrawerHistory = {
  commits: Array<{
    id: string;
    summary: string;
  }>;
};

type DrawerItem = {
  id: number;
  title: string;
  kind: string;
  storage_path: string | null;
  tags: string[];
  updated_at: string;
};

type HiveRun = {
  id: number;
  title: string;
  status: string;
  project_dir: string | null;
  summary: string | null;
  updated_at: string;
};

type HiveSummary = {
  total_runs: number;
  ready_runs: number;
  returned_runs: number;
};

type LauncherResult = {
  id: number;
  name: string;
  command: string;
  source: string;
  launch_count: number;
  pinned: boolean;
  score: number;
  arguments: string | null;
  working_dir: string | null;
};

export default function App() {
  const [view, setView] = createSignal<View>("status");
  const [launcherQuery, setLauncherQuery] = createSignal("");
  const [hiveTitle, setHiveTitle] = createSignal("");
  const [hiveProject, setHiveProject] = createSignal("");
  const [drawerTitle, setDrawerTitle] = createSignal("");
  const [drawerBody, setDrawerBody] = createSignal("");
  const [banner, setBanner] = createSignal<string>("");

  const [status, { refetch: refetchStatus }] = createResource(async () =>
    bridge.invoke<AppStatus>("status"),
  );
  const [drawerSummary, { refetch: refetchDrawerSummary }] = createResource(async () =>
    bridge.invoke<DrawerSummary>("drawer_summary"),
  );
  const [drawerItems, { refetch: refetchDrawerItems }] = createResource(async () =>
    bridge.invoke<DrawerItem[]>("drawer_list", {}),
  );
  const [drawerHistory, { refetch: refetchDrawerHistory }] = createResource(async () =>
    bridge.invoke<DrawerHistory>("drawer_history"),
  );
  const [hiveRuns, { refetch: refetchHiveRuns }] = createResource(async () =>
    bridge.invoke<HiveRun[]>("hive_list"),
  );
  const [hiveSummary, { refetch: refetchHiveSummary }] = createResource(async () =>
    bridge.invoke<HiveSummary>("hive_summary"),
  );
  const [launcherItems, { refetch: refetchLauncher }] = createResource(launcherQuery, async (query) =>
    bridge.invoke<LauncherResult[]>("launcher_search", { query, limit: 12 }),
  );

  const refreshAll = async () => {
    await Promise.all([
      refetchStatus(),
      refetchDrawerSummary(),
      refetchDrawerItems(),
      refetchDrawerHistory(),
      refetchHiveRuns(),
      refetchHiveSummary(),
      refetchLauncher(),
    ]);
  };

  const addDrawerNote = async () => {
    await bridge.invoke("drawer_add_note", {
      title: drawerTitle() || "Untitled Note",
      body: drawerBody(),
    });
    setDrawerTitle("");
    setDrawerBody("");
    setBanner("Drawer note created.");
    await Promise.all([refetchDrawerSummary(), refetchDrawerItems(), refetchDrawerHistory(), refetchStatus()]);
  };

  const dispatchHive = async () => {
    await bridge.invoke("hive_dispatch", {
      title: hiveTitle() || "Untitled dispatch",
      projectDir: hiveProject() || undefined,
    });
    setHiveTitle("");
    setHiveProject("");
    setBanner("Hive dispatch persisted.");
    await Promise.all([refetchHiveRuns(), refetchHiveSummary(), refetchStatus()]);
  };

  const refreshLauncherIndex = async () => {
    await bridge.invoke("launcher_refresh", {});
    setBanner("Launcher index refreshed.");
    await Promise.all([refetchLauncher(), refetchStatus()]);
  };

  const launchItem = async (item: LauncherResult) => {
    await bridge.invoke("launcher_launch", {
      command: item.command,
      arguments: item.arguments,
      workingDir: item.working_dir,
    });
    setBanner(`Launched ${item.name}.`);
    await refetchLauncher();
  };

  const pinItem = async (item: LauncherResult) => {
    await bridge.invoke("launcher_pin", {
      command: item.command,
      pinned: !item.pinned,
    });
    await refetchLauncher();
  };

  return (
    <div class="app-shell">
      <Nav current={view()} onSelect={setView} />

      <main class="main-shell">
        <header class="hero-panel">
          <div>
            <p class="hero-kicker">Refactor target</p>
            <h2>Core / Plugins / Shell</h2>
            <p class="hero-copy">
              This GUI talks only to the unified `entrance daemon` protocol.
            </p>
          </div>
          <button type="button" class="hero-action" onClick={() => void refreshAll()}>
            Refresh
          </button>
        </header>

        {banner() ? <p class="banner">{banner()}</p> : null}

        <Switch>
          <Match when={view() === "status"}>
            <section class="panel-grid panel-grid--status">
              <article class="panel">
                <p class="panel-kicker">Kernel</p>
                <h3>Runtime status</h3>
                <dl class="metric-list">
                  <div><dt>App root</dt><dd>{status()?.app_root ?? "..."}</dd></div>
                  <div><dt>Database</dt><dd>{status()?.db_path ?? "..."}</dd></div>
                  <div><dt>Drawer</dt><dd>{status()?.drawer_entries ?? 0}</dd></div>
                  <div><dt>Hive</dt><dd>{status()?.hive_runs ?? 0}</dd></div>
                  <div><dt>Launcher</dt><dd>{status()?.launcher_entries ?? 0}</dd></div>
                </dl>
              </article>

              <article class="panel">
                <p class="panel-kicker">Drawer</p>
                <h3>Storage mode</h3>
                <p class="big-copy">{drawerSummary()?.mode ?? "..."}</p>
                <p class="muted">{drawerSummary()?.root ?? "..."}</p>
              </article>

              <article class="panel">
                <p class="panel-kicker">Identity</p>
                <h3>Microkernel cutover</h3>
                <p class="muted">
                  Runtime, daemon, and GUI now share the same single-binary command contract.
                </p>
              </article>
            </section>
          </Match>

          <Match when={view() === "drawer"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Drawer</p>
                <h3>Add note</h3>
                <input
                  value={drawerTitle()}
                  onInput={(event) => setDrawerTitle(event.currentTarget.value)}
                  placeholder="Title"
                />
                <textarea
                  value={drawerBody()}
                  onInput={(event) => setDrawerBody(event.currentTarget.value)}
                  placeholder="Write a note for the drawer"
                />
                <button type="button" class="primary-button" onClick={() => void addDrawerNote()}>
                  Create Note
                </button>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Items</p>
                <h3>Stored entries</h3>
                <ul class="record-list">
                  {(drawerItems() ?? []).map((item) => (
                    <li class="record-card">
                      <strong>{item.title}</strong>
                      <span>{item.kind}</span>
                      <code>{item.storage_path ?? "db-only"}</code>
                    </li>
                  ))}
                </ul>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Versioning</p>
                <h3>Drawer history</h3>
                <ul class="record-list">
                  {(drawerHistory()?.commits ?? []).map((commit) => (
                    <li class="record-card">
                      <strong>{commit.summary}</strong>
                      <code>{commit.id}</code>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>

          <Match when={view() === "hive"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Hive</p>
                <h3>Dispatch</h3>
                <input
                  value={hiveTitle()}
                  onInput={(event) => setHiveTitle(event.currentTarget.value)}
                  placeholder="Task title"
                />
                <input
                  value={hiveProject()}
                  onInput={(event) => setHiveProject(event.currentTarget.value)}
                  placeholder="Project path (optional)"
                />
                <button type="button" class="primary-button" onClick={() => void dispatchHive()}>
                  Persist Dispatch
                </button>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Runs</p>
                <h3>Dispatch ledger</h3>
                <p class="muted">
                  Ready {hiveSummary()?.ready_runs ?? 0} / Total {hiveSummary()?.total_runs ?? 0}
                </p>
                <ul class="record-list">
                  {(hiveRuns() ?? []).map((run) => (
                    <li class="record-card">
                      <strong>{run.title}</strong>
                      <span>{run.status}</span>
                      <code>{run.project_dir ?? "no project"}</code>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>

          <Match when={view() === "launcher"}>
            <section class="panel-grid">
              <article class="panel panel--form">
                <p class="panel-kicker">Launcher</p>
                <h3>Search index</h3>
                <input
                  value={launcherQuery()}
                  onInput={(event) => setLauncherQuery(event.currentTarget.value)}
                  placeholder="Search indexed apps"
                />
                <button type="button" class="primary-button" onClick={() => void refreshLauncherIndex()}>
                  Refresh Index
                </button>
                <p class="muted">Launcher routes through the unified daemon contract.</p>
              </article>

              <article class="panel panel--list">
                <p class="panel-kicker">Matches</p>
                <h3>Launch surface</h3>
                <ul class="record-list">
                  {(launcherItems() ?? []).map((item) => (
                    <li class="record-card">
                      <div class="record-head">
                        <strong>{item.name}</strong>
                        <span>{item.score.toFixed(2)}</span>
                      </div>
                      <code>{item.command}</code>
                      <div class="record-actions">
                        <button type="button" onClick={() => void launchItem(item)}>
                          Launch
                        </button>
                        <button type="button" onClick={() => void pinItem(item)}>
                          {item.pinned ? "Unpin" : "Pin"}
                        </button>
                      </div>
                    </li>
                  ))}
                </ul>
              </article>
            </section>
          </Match>
        </Switch>
      </main>
    </div>
  );
}
