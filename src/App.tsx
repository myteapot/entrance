import { For, onCleanup, onMount, type Component } from "solid-js";
import { Route, Router, type RouteSectionProps, useNavigate } from "@solidjs/router";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import "./App.css";
import MainPanel from "./components/MainPanel";
import {
  getVaultTokenByProvider,
  GITLAB_UPDATER_PROVIDER,
} from "./features/vault/client";
import Sidebar from "./components/Sidebar";
import { appRoutes, shortcutRoutes } from "./router";

const PUBLIC_UPDATER_ENABLED = import.meta.env.VITE_ENABLE_UPDATER === "true";

const isEditableTarget = (target: EventTarget | null) => {
  if (!(target instanceof HTMLElement)) {
    return false;
  }

  const tagName = target.tagName.toLowerCase();

  return (
    tagName === "input" ||
    tagName === "textarea" ||
    tagName === "select" ||
    target.isContentEditable
  );
};

const checkForAppUpdates = async () => {
  if (import.meta.env.DEV || !PUBLIC_UPDATER_ENABLED) {
    return;
  }

  try {
    let updaterHeaders: Record<string, string> | undefined;
    try {
      const gitlabToken = await getVaultTokenByProvider(GITLAB_UPDATER_PROVIDER);
      if (gitlabToken?.value) {
        updaterHeaders = {
          "PRIVATE-TOKEN": gitlabToken.value,
        };
      }
    } catch (error) {
      console.warn("Failed to load GitLab updater token from Vault", error);
    }

    const update = await check(
      updaterHeaders
        ? {
            headers: updaterHeaders,
          }
        : undefined,
    );

    if (!update) {
      return;
    }

    const details = update.body?.trim();
    const shouldInstall = await ask(
      details
        ? `发现新版本 ${update.version}。\n\n更新说明:\n${details}\n\n是否现在下载并重启安装？`
        : `发现新版本 ${update.version}，是否现在下载并重启安装？`,
      {
        title: "Entrance 有可用更新",
        kind: "info",
        okLabel: "立即更新",
        cancelLabel: "稍后"
      }
    );

    if (!shouldInstall) {
      return;
    }

    await update.downloadAndInstall(undefined, {
      headers: updaterHeaders,
    });
    await relaunch();
  } catch (error) {
    const description =
      error instanceof Error ? error.message : "未知错误，请检查 updater 配置与服务器可达性。";

    await message(`检查或安装更新失败：${description}`, {
      title: "更新失败",
      kind: "error",
      buttons: { ok: "知道了" }
    });
  }
};

const AppShell: Component<RouteSectionProps> = (props) => {
  const navigate = useNavigate();

  onMount(() => {
    void checkForAppUpdates();

    const handleKeydown = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) {
        return;
      }

      if (isEditableTarget(event.target)) {
        return;
      }

      const shortcutIndex = Number(event.key) - 1;
      const route = shortcutRoutes[shortcutIndex];

      if (!route) {
        return;
      }

      event.preventDefault();
      navigate(route.path);
    };

    window.addEventListener("keydown", handleKeydown);
    onCleanup(() => window.removeEventListener("keydown", handleKeydown));
  });

  return (
    <div class="app-shell">
      <Sidebar />
      <MainPanel>{props.children}</MainPanel>
    </div>
  );
};

function App() {
  return (
    <Router root={AppShell}>
      <For each={appRoutes}>
        {(route) => <Route path={route.path} component={route.component} />}
      </For>
    </Router>
  );
}

export default App;
