import { resolve } from "node:path";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

const host = process.env.TAURI_DEV_HOST;
const repoRoot = resolve(__dirname, "../..");

// When running plain `pnpm dev` (no Tauri backend), redirect all Tauri
// imports to local mocks so the full UI renders in the browser.
const isTauri = !!process.env.TAURI_ENV_PLATFORM;
const mockAliases: Record<string, string> = isTauri
  ? {}
  : {
      "@tauri-apps/api/core": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-core.ts"),
      "@tauri-apps/api/event": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-event.ts"),
      "@tauri-apps/api/window": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-window.ts"),
      "@tauri-apps/plugin-dialog": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-plugin-dialog.ts"),
      "@tauri-apps/plugin-process": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-plugin-process.ts"),
      "@tauri-apps/plugin-updater": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-plugin-updater.ts"),
      "@tauri-apps/plugin-opener": resolve(repoRoot, "hosts/desktop/browser/mocks/tauri-plugin-opener.ts"),
    };

// https://vite.dev/config/
export default defineConfig({
  root: __dirname,
  base: "./",
  plugins: [solid()],
  build: {
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        launcher: resolve(__dirname, "launcher.html"),
      },
    },
  },
  resolve: {
    alias: {
      "@": resolve(__dirname, "renderer"),
      "@desktop": resolve(repoRoot, "surfaces/contracts/desktop"),
      ...mockAliases,
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `hosts/desktop/tauri`
      ignored: ["**/hosts/desktop/tauri/**"],
    },
  },
  preview: {
    port: 1420,
    strictPort: true,
  },
});
