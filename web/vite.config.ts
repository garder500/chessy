import react from "@vitejs/plugin-react";
import { loadEnv } from "vite";
import { defineConfig, type Plugin } from "vitest/config";

// index.html porte des marqueurs {{…}} que le serveur Rust remplit (crates/chessy-server/src/seo.rs) ;
// en développement, Vite sert la page seul : on met des valeurs par défaut.
const devMeta: Record<string, string> = {
  LANG: "fr",
  OG_LOCALE: "fr_FR",
  TITLE: "Chessy",
  DESCRIPTION: "Les échecs en ligne, avec des compétences.",
  IMAGE_ALT: "Chessy",
  SITE_URL: "",
};
const seoDefaults = (): Plugin => ({
  name: "seo-defaults",
  apply: "serve",
  transformIndexHtml: (html) => html.replace(/\{\{(\w+)\}\}/g, (m, k: string) => devMeta[k] ?? m),
});

// In development the Rust server runs on CHESSY_ADDR (default :3000) and Vite proxies the socket to it.
export default defineConfig(({ mode }) => {
  const serverAddr = loadEnv(mode, ".", "CHESSY_").CHESSY_ADDR ?? "127.0.0.1:3000";
  return {
    plugins: [react(), seoDefaults()],
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
