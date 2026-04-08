import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

import { _electron as electron } from "playwright";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const repoRoot = path.resolve(__dirname, "..", "..");
const rendererUrl = process.env.ENTRANCE_RENDERER_URL ?? "http://127.0.0.1:1420";

const spawnChild = (command, args, options = {}) =>
  spawn(command, args, {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
    shell: process.platform === "win32",
    ...options,
  });

const attachLogs = (name, child) => {
  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    process.stdout.write(`[${name}] ${chunk}`);
  });
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    process.stderr.write(`[${name}] ${chunk}`);
  });
};

const waitForExit = async (child, timeoutMs) => {
  if (child.exitCode !== null) {
    return true;
  }

  return new Promise((resolve) => {
    const timer = setTimeout(() => {
      cleanup();
      resolve(false);
    }, timeoutMs);
    const onExit = () => {
      cleanup();
      resolve(true);
    };
    const cleanup = () => {
      clearTimeout(timer);
      child.removeListener("exit", onExit);
    };

    child.once("exit", onExit);
  });
};

const stopChild = async (child, name) => {
  if (!child || child.killed) {
    return;
  }

  child.kill("SIGTERM");
  if (await waitForExit(child, 10_000)) {
    return;
  }

  child.kill("SIGKILL");
  if (!(await waitForExit(child, 5_000))) {
    throw new Error(`${name} did not exit after SIGKILL`);
  }
};

const waitForRenderer = async (url) => {
  const deadline = Date.now() + 90_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { method: "GET" });
      if (response.ok) {
        return;
      }
    } catch {
      // keep polling until the Vite server is ready
    }
    await delay(500);
  }

  throw new Error(`Timed out waiting for renderer at ${url}`);
};

const sidebarLink = (window, label) =>
  window.locator(".sidebar__link", {
    has: window.locator(".sidebar__label", {
      hasText: new RegExp(`^${label}$`),
    }),
  });

const sweepRoute = async (window, label, expectedPath) => {
  await sidebarLink(window, label).click();
  await window.waitForURL(expectedPath);
};

const runSmoke = async () => {
  if (
    process.platform !== "win32" &&
    !process.env.DISPLAY &&
    !process.env.WAYLAND_DISPLAY
  ) {
    throw new Error(
      "No display server detected. Run with xvfb (for example: xvfb-run -a pnpm test:electron-smoke).",
    );
  }

  const vite = spawnChild("pnpm", ["exec", "vite", "--host", "127.0.0.1", "--port", "1420"]);
  attachLogs("vite", vite);

  let electronApp;
  try {
    await waitForRenderer(rendererUrl);

    electronApp = await electron.launch({
      args: [path.join(repoRoot, "electron", "main.mjs")],
      cwd: repoRoot,
      env: {
        ...process.env,
        ENTRANCE_RENDERER_URL: rendererUrl,
        ENTRANCE_ELECTRON_NO_DEVTOOLS: "1",
      },
    });

    const window = await electronApp.firstWindow();
    await window.waitForLoadState("domcontentloaded");

    await window.locator(".app-shell").waitFor({ state: "visible", timeout: 30_000 });

    await sweepRoute(window, "Do", /\/do$/);
    await sweepRoute(window, "Board", /\/board$/);
    await sweepRoute(window, "Issues", /\/issues$/);
    await sweepRoute(window, "Settings", /\/settings$/);
    await sweepRoute(window, "Chat", /\/$/);

    const hotkey = await window.evaluate(async () => {
      const bridge = window.__ENTRANCE_ELECTRON__;
      if (!bridge?.invoke) {
        throw new Error("Electron bridge invoke is unavailable");
      }
      return bridge.invoke("launcher_hotkey");
    });
    assert.ok(
      hotkey === null || typeof hotkey === "string",
      "launcher_hotkey should resolve to string or null",
    );

    const eventResult = await window.evaluate(async () => {
      const bridge = window.__ENTRANCE_ELECTRON__;
      if (!bridge?.invoke || !bridge?.listen) {
        throw new Error("Electron bridge invoke/listen is unavailable");
      }

      const observed = [];
      const unlisten = await bridge.listen("graph:update", (event) => {
        observed.push(event.payload);
      });

      let created;
      try {
        created = await bridge.invoke("create_agent_instance", {
          role: "dev",
          displayName: "electron-smoke-agent",
          configJson: "{}",
        });

        if (!created || typeof created.id !== "number") {
          throw new Error("create_agent_instance returned an invalid payload");
        }

        const expectedNodeId = `instance-${created.id}`;
        const deadline = Date.now() + 10_000;
        let matched = null;

        while (Date.now() < deadline && !matched) {
          for (const payload of observed) {
            try {
              const parsed = typeof payload === "string" ? JSON.parse(payload) : payload;
              if (parsed && parsed.id === expectedNodeId) {
                matched = parsed;
                break;
              }
            } catch {
              // ignore malformed event payloads
            }
          }

          if (!matched) {
            await new Promise((resolve) => setTimeout(resolve, 100));
          }
        }

        if (!matched) {
          throw new Error(`did not observe graph:update for ${expectedNodeId}`);
        }

        return {
          createdId: created.id,
          eventKind: matched.kind ?? null,
        };
      } finally {
        if (created && typeof created.id === "number") {
          await bridge.invoke("stop_agent_instance", { id: created.id });
        }
        if (typeof unlisten === "function") {
          await Promise.resolve(unlisten());
        }
      }
    });

    assert.equal(typeof eventResult.createdId, "number");
    assert.ok(
      typeof eventResult.eventKind === "string" || eventResult.eventKind === null,
      "graph:update payload should include a kind marker",
    );

    await electronApp.close();
    electronApp = null;
  } finally {
    if (electronApp) {
      await electronApp.close();
    }
    await stopChild(vite, "vite");
  }
};

runSmoke()
  .then(() => {
    console.log("[electron-smoke] success");
  })
  .catch((error) => {
    console.error("[electron-smoke] failed:", error);
    process.exitCode = 1;
  });
