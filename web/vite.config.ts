import react from "@vitejs/plugin-react";
import { loadEnv } from "vite";
import { defineConfig } from "vitest/config";

// In development the Rust server runs on CHESSY_ADDR (default :3000) and Vite proxies the socket to it.
export default defineConfig(({ mode }) => {
  const serverAddr = loadEnv(mode, ".", "CHESSY_").CHESSY_ADDR ?? "127.0.0.1:3000";
  return {
    plugins: [react()],
    server: {
      proxy: {
        "/ws": { target: `ws://${serverAddr}`, ws: true },
        "/api": { target: `http://${serverAddr}` },
      },
    },
    build: {
      chunkSizeWarningLimit: 1800, // the lazy-loaded Phaser chunk is ~1.6 MB
    },
  };
});
