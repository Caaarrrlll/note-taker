import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri-friendly Vite config: fixed port. Root is web/; output goes to web/dist.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    // `npm run dev:web`: forward API calls to a local note-server (cargo run -p note-server).
    proxy: {
      "/api": "http://localhost:8080",
      "/healthz": "http://localhost:8080",
    },
  },
  build: {
    target: "es2022",
    outDir: "dist",
    // Loaded from disk by the desktop app, so bundle size is not a concern.
    chunkSizeWarningLimit: 4000,
  },
});
