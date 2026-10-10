// Génère web/public/og-image.png (1200x630) depuis og-image.html.
// Usage : node scripts/og/render.mjs   (nécessite playwright, non installé en dépendance du projet)
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";
import path from "node:path";

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE ?? "playwright");
const here = path.dirname(fileURLToPath(import.meta.url));
const out = path.resolve(here, "../../public/og-image.png");

const browser = await chromium.launch(
  process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
);
const page = await browser.newPage({ viewport: { width: 1200, height: 630 } });
await page.goto(pathToFileURL(path.join(here, "og-image.html")).href);
await page.evaluate(() => document.fonts.ready);
await page.screenshot({ path: out });
await browser.close();
console.log("écrit", out);
