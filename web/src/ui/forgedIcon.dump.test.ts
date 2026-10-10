// @ts-nocheck
// Outil, pas un test : `ICON_DUMP=/chemin/icones.json npx vitest run src/ui/forgedIcon.dump.test.ts` écrit des icônes
// tirées au hasard (SVG + briques) pour `scripts/icon-metrics.mjs`, qui les mesure à 32 px dans Chromium.
import { writeFileSync } from "node:fs";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { it } from "vitest";
import type { Family } from "../catalog";
import type { IconSpec, Rarity } from "../forged";
import { FAMILY_BG, ForgedBadge, GLYPH_NAMES, rng } from "./forgedIcon";

const out = process.env.ICON_DUMP;

it.skipIf(!out)("écrit les icônes à mesurer", () => {
  const r = rng(2024);
  const pick = <T>(a: readonly T[]): T => a[Math.floor(r() * a.length)];
  const families = Object.keys(FAMILY_BG) as Family[];
  const rarities: Rarity[] = ["common", "uncommon", "rare", "epic", "legendary"];
  const N = Number(process.env.ICON_N ?? 400);
  const items = Array.from({ length: N }, () => {
    const glyph = pick(GLYPH_NAMES);
    const instant = ["crown", "erase", "banner", "portal", "echo", "swap", "ankh", "mirror"].includes(glyph);
    const icon: IconSpec = {
      glyph,
      piece: pick(["pawn", "knight", "bishop", "rook", "queen"] as const),
      target: pick(["own", "enemy"] as const),
      plies: instant ? undefined : 2 + Math.floor(r() * 7),
      zone: pick(["one", "row", "board"] as const),
      mark: pick([undefined, undefined, "free", "check", "safe"] as const),
      seed: Math.floor(r() * 2 ** 32),
    };
    const family = pick(families);
    const rarity = pick(rarities);
    const svg = renderToStaticMarkup(createElement("svg", { xmlns: "http://www.w3.org/2000/svg", viewBox: "0 0 120 120", width: 120, height: 120 }, createElement(ForgedBadge, { icon, family, rarity })));
    return { svg, labels: { verb: glyph, kind: icon.piece, camp: icon.target, zone: icon.zone, plies: String(icon.plies ?? "none"), mark: icon.mark ?? "none", rarity } };
  });
  writeFileSync(out!, JSON.stringify(items));

  // Paires qui ne diffèrent que d'une brique : de combien l'image bouge-t-elle ?
  const draw = (icon: IconSpec, family: Family, rarity: Rarity) =>
    renderToStaticMarkup(createElement("svg", { xmlns: "http://www.w3.org/2000/svg", viewBox: "0 0 120 120", width: 120, height: 120 }, createElement(ForgedBadge, { icon, family, rarity })));
  const pairs: { brick: string; a: string; b: string }[] = [];
  for (let i = 0; i < 150; i++) {
    const glyph = pick(GLYPH_NAMES);
    const instant = ["crown", "erase", "banner", "portal", "echo", "swap", "ankh", "mirror"].includes(glyph);
    const base: IconSpec = { glyph, piece: "knight", target: "own", plies: instant ? undefined : 4, zone: "one", seed: Math.floor(r() * 2 ** 32) };
    const family = pick(families);
    const rarity = pick(rarities);
    const a = draw(base, family, rarity);
    const variants: [string, IconSpec, Rarity?][] = [
      ["zone", { ...base, zone: "row" }],
      ["zone", { ...base, zone: "board" }],
      ["kind", { ...base, piece: "rook" }],
      ["camp", { ...base, target: "enemy" }],
      ["mark", { ...base, mark: "free" }],
      ["mark", { ...base, mark: "check" }],
      ["seed", { ...base, seed: base.seed! + 7919 }],
      ["rarity", base, pick(rarities.filter((x) => x !== rarity))],
    ];
    if (!instant) variants.push(["plies", { ...base, plies: 5 }], ["plies", { ...base, plies: 2 }], ["plies", { ...base, plies: 8 }]);
    for (const [brick, icon, rar] of variants) pairs.push({ brick, a, b: draw(icon, family, rar ?? rarity) });
  }
  writeFileSync(out!.replace(/\.json$/, "") + ".pairs.json", JSON.stringify(pairs));
});
