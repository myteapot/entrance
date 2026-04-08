import { spawn } from "node:child_process";

const rendererUrl = process.env.ENTRANCE_RENDERER_URL ?? "http://127.0.0.1:1420";

const spawnChild = (command, args, options = {}) =>
  spawn(command, args, {
    stdio: "inherit",
    shell: process.platform === "win32",
    ...options,
  });

const waitForRenderer = async (url) => {
  const deadline = Date.now() + 60_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url, { method: "GET" });
      if (response.ok) {
        return;
      }
    } catch {
      // Keep polling until the Vite server comes up or the deadline expires.
    }

    await new Promise((resolve) => setTimeout(resolve, 500));
  }

  throw new Error(`Timed out waiting for renderer at ${url}`);
};

const vite = spawnChild("pnpm", ["exec", "vite", "--host", "127.0.0.1", "--port", "1420"]);
let electron;

const shutdown = (code = 0) => {
  if (electron && !electron.killed) {
    electron.kill();
  }
  if (!vite.killed) {
    vite.kill();
  }
  process.exit(code);
};

vite.on("exit", (code) => {
  if (electron && !electron.killed) {
    electron.kill();
  }
  process.exit(code ?? 0);
});

process.on("SIGINT", () => shutdown(0));
process.on("SIGTERM", () => shutdown(0));

await waitForRenderer(rendererUrl);

electron = spawnChild(
  "pnpm",
  ["exec", "electron", "electron/main.mjs"],
  {
    env: {
      ...process.env,
      ENTRANCE_RENDERER_URL: rendererUrl,
    },
  },
);

electron.on("exit", (code) => shutdown(code ?? 0));

