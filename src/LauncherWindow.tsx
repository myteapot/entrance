import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  For,
  Show,
  createEffect,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import "./LauncherWindow.css";

type LauncherSearchResult = {
  id: number;
  name: string;
  path: string;
  arguments: string | null;
  working_dir: string | null;
  icon_path: string | null;
  source: string;
  launch_count: number;
  last_used: string | null;
  pinned: boolean;
  score: number;
};

const launcherWindow = getCurrentWindow();
const SEARCH_LIMIT = 8;
const HIDE_DELAY_MS = 140;

function formatHotkeyLabel(shortcut: string | null | undefined) {
  return shortcut?.split("+").join(" + ") ?? "No shortcut";
}

function sourceLabel(source: string) {
  if (source === "windows_start_menu") {
    return "Start Menu";
  }

  if (source === "windows_registry") {
    return "Windows Registry";
  }

  return source.split("_").join(" ");
}

function LauncherWindow() {
  const [query, setQuery] = createSignal("");
  const [results, setResults] = createSignal<LauncherSearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = createSignal(0);
  const [isVisible, setIsVisible] = createSignal(false);
  const [isSearching, setIsSearching] = createSignal(false);
  const [isLaunching, setIsLaunching] = createSignal(false);
  const [errorMessage, setErrorMessage] = createSignal<string>();
  const [launcherHotkeyLabel, setLauncherHotkeyLabel] =
    createSignal("Loading shortcut...");

  let inputRef: HTMLInputElement | undefined;
  let searchRunId = 0;
  let visibilityRunId = 0;

  const focusInput = () => {
    inputRef?.focus();
    inputRef?.select();
  };

  const resetSelection = () => {
    setSelectedIndex(0);
  };

  const runSearch = async (nextQuery: string) => {
    const runId = ++searchRunId;
    setIsSearching(true);
    setErrorMessage(undefined);

    try {
      const nextResults = await invoke<LauncherSearchResult[]>("launcher_search", {
        query: nextQuery,
        limit: SEARCH_LIMIT,
      });

      if (runId !== searchRunId) {
        return;
      }

      setResults(nextResults);
      resetSelection();
    } catch (error) {
      if (runId !== searchRunId) {
        return;
      }

      setResults([]);
      setSelectedIndex(0);
      setErrorMessage(
        error instanceof Error ? error.message : "搜索失败，请稍后再试。",
      );
    } finally {
      if (runId === searchRunId) {
        setIsSearching(false);
      }
    }
  };

  const showLauncher = async () => {
    visibilityRunId += 1;
    setErrorMessage(undefined);
    setIsVisible(true);

    await launcherWindow.show();
    await launcherWindow.center();
    await launcherWindow.setFocus();
    focusInput();
  };

  const hideLauncher = async (resetQuery = false) => {
    const runId = ++visibilityRunId;
    setIsVisible(false);

    if (resetQuery) {
      setQuery("");
      setResults([]);
      setSelectedIndex(0);
      setErrorMessage(undefined);
    }

    window.setTimeout(async () => {
      if (runId !== visibilityRunId) {
        return;
      }

      await launcherWindow.hide();
    }, HIDE_DELAY_MS);
  };

  const launchSelection = async (item: LauncherSearchResult | undefined) => {
    if (!item || isLaunching()) {
      return;
    }

    setIsLaunching(true);
    setErrorMessage(undefined);

    try {
      await invoke("launcher_launch", {
        path: item.path,
        arguments: item.arguments,
        workingDir: item.working_dir,
      });
      await hideLauncher(true);
    } catch (error) {
      setErrorMessage(
        error instanceof Error ? error.message : "启动失败，请稍后重试。",
      );
    } finally {
      setIsLaunching(false);
    }
  };

  createEffect(() => {
    const nextQuery = query();
    if (!isVisible()) {
      return;
    }

    void runSearch(nextQuery);
  });

  onMount(async () => {
    setIsVisible(await launcherWindow.isVisible());
    try {
      const shortcut = await invoke<string | null>("launcher_hotkey");
      setLauncherHotkeyLabel(formatHotkeyLabel(shortcut));
    } catch {
      setLauncherHotkeyLabel("No shortcut");
    }

    const unlistenToggle = await launcherWindow.listen("launcher:toggle", async () => {
      if (await launcherWindow.isVisible()) {
        await hideLauncher(true);
        return;
      }

      await showLauncher();
    });

    const unlistenFocus = await launcherWindow.onFocusChanged(async ({ payload }) => {
      if (!payload) {
        await hideLauncher(true);
      }
    });

    onCleanup(() => {
      void unlistenToggle();
      void unlistenFocus();
    });

    if (isVisible()) {
      focusInput();
    }
  });

  const handleInputKeyDown = async (event: KeyboardEvent) => {
    const nextResults = results();

    if (event.key === "ArrowDown" && nextResults.length > 0) {
      event.preventDefault();
      setSelectedIndex((current) => (current + 1) % nextResults.length);
      return;
    }

    if (event.key === "ArrowUp" && nextResults.length > 0) {
      event.preventDefault();
      setSelectedIndex(
        (current) => (current - 1 + nextResults.length) % nextResults.length,
      );
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      await launchSelection(nextResults[selectedIndex()]);
      return;
    }

    if (event.key === "Escape") {
      event.preventDefault();
      await hideLauncher(true);
    }
  };

  return (
    <main class={`launcher-shell ${isVisible() ? "is-visible" : ""}`}>
      <section class="launcher-panel" aria-label="Launcher search window">
        <header class="launcher-header">
          <div>
            <p class="launcher-eyebrow">{launcherHotkeyLabel()}</p>
            <h1>Launcher</h1>
          </div>
          <p class="launcher-tip">Enter 启动 · Esc 收起</p>
        </header>

        <label class="search-field">
          <span class="search-icon" aria-hidden="true">
            /
          </span>
          <input
            ref={inputRef}
            type="text"
            value={query()}
            placeholder="搜索本地应用…"
            spellcheck={false}
            onInput={(event) => setQuery(event.currentTarget.value)}
            onKeyDown={(event) => {
              void handleInputKeyDown(event);
            }}
          />
        </label>

        <Show when={errorMessage()}>
          {(message) => <p class="status-line is-error">{message()}</p>}
        </Show>

        <Show when={isSearching()}>
          <p class="status-line">正在刷新结果…</p>
        </Show>

        <ul class="result-list">
          <Show
            when={results().length > 0}
            fallback={
              <li class="empty-state">
                <p>没有匹配结果</p>
                <span>试试应用名、品牌名，或先留空浏览常用应用。</span>
              </li>
            }
          >
            <For each={results()}>
              {(item, index) => (
                <li>
                  <button
                    type="button"
                    class={`result-card ${index() === selectedIndex() ? "is-selected" : ""}`}
                    onMouseEnter={() => setSelectedIndex(index())}
                    onClick={() => {
                      void launchSelection(item);
                    }}
                  >
                    <div class="result-main">
                      <div class="result-title-row">
                        <span class="result-title">{item.name}</span>
                        <Show when={item.pinned}>
                          <span class="result-pill">Pinned</span>
                        </Show>
                      </div>
                      <p class="result-subtitle">{item.path}</p>
                    </div>

                    <div class="result-meta">
                      <span>{sourceLabel(item.source)}</span>
                      <span>{item.launch_count} launches</span>
                    </div>
                  </button>
                </li>
              )}
            </For>
          </Show>
        </ul>
      </section>
    </main>
  );
}

export default LauncherWindow;
