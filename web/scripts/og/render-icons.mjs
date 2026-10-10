// Génère les icônes PNG (web/public) depuis public/favicon.svg.
// Usage : node scripts/og/render-icons.mjs   (voir render.mjs pour PLAYWRIGHT_MODULE)
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import fs from "node:fs";
import path from "node:path";

const require = createRequire(import.meta.url);
const { chromium } = require(process.env.PLAYWRIGHT_MODULE ?? "playwright");
const pub = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../public");
const svg = fs.readFileSync(path.join(pub, "favicon.svg"), "utf8");

const browser = await chromium.launch(
  process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {},
);
for (const [name, size] of [["icon-192.png", 192], ["icon-512.png", 512], ["apple-touch-icon.png", 180]]) {
  const page = await browser.newPage({ viewport: { width: size, height: size } });
  await page.setContent(`<body style="margin:0;background:#0f1314">${svg.replace("<svg ", `<svg width="${size}" height="${size}" `)}</body>`);
  await page.screenshot({ path: path.join(pub, name) });
  await page.close();
}
await browser.close();
