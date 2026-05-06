import { resolve } from "node:path";
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  root: __dirname,
  base: "./",
  plugins: [solid()],
  build: {
    outDir: "dist",
  },
  resolve: {
    alias: {
      "@": resolve(__dirname, "renderer"),
    },
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: {
      ignored: ["**/target/**", "**/dist/**"],
    },
  },
  preview: {
    port: 1420,
    strictPort: true,
  },
});
