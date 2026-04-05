import { resolve } from "node:path";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

const host = process.env.TAURI_DEV_HOST;

// When running plain `pnpm dev` (no Tauri backend), redirect all Tauri
// imports to local mocks so the full UI renders in the browser.
const isTauri = !!process.env.TAURI_ENV_PLATFORM;
const mockAliases: Record<string, string> = isTauri
  ? {}
  : {
      "@tauri-apps/api/core": resolve(__dirname, "src/mocks/tauri-core.ts"),
      "@tauri-apps/api/event": resolve(__dirname, "src/mocks/tauri-event.ts"),
      "@tauri-apps/api/window": resolve(__dirname, "src/mocks/tauri-window.ts"),
      "@tauri-apps/plugin-dialog": resolve(__dirname, "src/mocks/tauri-plugin-dialog.ts"),
      "@tauri-apps/plugin-process": resolve(__dirname, "src/mocks/tauri-plugin-process.ts"),
      "@tauri-apps/plugin-updater": resolve(__dirname, "src/mocks/tauri-plugin-updater.ts"),
      "@tauri-apps/plugin-opener": resolve(__dirname, "src/mocks/tauri-plugin-opener.ts"),
    };

// https://vite.dev/config/
export default defineConfig({
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
      "@": resolve(__dirname, "src"),
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
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  preview: {
    port: 1420,
    strictPort: true,
  },
});
