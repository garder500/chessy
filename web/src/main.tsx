import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initTheme } from "./theme";
import "./styles/tokens.css";
import "./styles/base.css";
import "./ui/shell.css";
import "./ui/skill.css";
import "./styles/color.css";
import "./styles/skin.css";

initTheme();

async function boot() {
  // Serveur simulé (fixtures) : `?mock=1`, développement uniquement. Le bloc disparaît du bundle de production.
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("mock")) {
    (await import("./dev/mock")).installMock();
  }
  // Console de développement : `__store.set({ reveal: "mirror" })` montre la révélation de forge.
  if (import.meta.env.DEV) (window as unknown as { __store: unknown }).__store = (await import("./store")).store;
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}

void boot();
