import { resolve } from "node:path";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

// Tauri expects a fixed port and does not need Vite to clear the terminal.
export default defineConfig({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "safari17",
    rollupOptions: {
      input: { main: resolve(__dirname, "index.html"), banner: resolve(__dirname, "banner.html") },
    },
  },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
