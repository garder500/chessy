import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Family } from "../catalog";
import type { Rarity } from "../forged";
import { FAMILY_FLAT, ForgedBadge, GLYPH_NAMES, INK_COLORS, RARITY_EDGE, SIGNS } from "./forgedIcon";

const RARITIES: Rarity[] = ["common", "uncommon", "rare", "epic", "legendary"];
const FAMILIES = Object.keys(FAMILY_FLAT) as Family[];

const render = (glyph: string, family: Family = "control", rarity: Rarity = "rare") =>
  renderToStaticMarkup(createElement("svg", null, createElement(ForgedBadge, { glyph, family, rarity })));

/** Luminance relative WCAG d'une couleur #rrggbb. */
function luminance(hex: string): number {
  const n = parseInt(hex.slice(1), 16);
  const c = [n >> 16, (n >> 8) & 255, n & 255].map((v) => {
    const s = v / 255;
    return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
}
const contrast = (a: string, b: string) => {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
};

describe("icône forgée : un signe à plat", () => {
  it("dix-sept signes, un par verbe, sans symbole externe", () => {
    expect(GLYPH_NAMES).toHaveLength(17);
    for (const glyph of GLYPH_NAMES) {
      const html = render(glyph);
      expect(html, glyph).toContain("<path");
      expect(html).not.toContain("<use");
    }
  });

  it("chaque signe tient en deux ou trois couleurs au plus, rien de plus", () => {
    for (const glyph of GLYPH_NAMES) {
      const roles = new Set(SIGNS[glyph].map(([role]) => role));
      expect(roles.size, glyph).toBeLessThanOrEqual(3);
      // Le creux sombre ne compte pas comme une couleur : c'est le fond qui revient.
      expect(SIGNS[glyph].length, glyph).toBeLessThanOrEqual(4);
    }
  });

  it("deux verbes différents ne se dessinent jamais pareil", () => {
    expect(new Set(GLYPH_NAMES.map((g) => render(g))).size).toBe(GLYPH_NAMES.length);
  });

  it("la rareté ne change que le contour : cinq formes, cinq couleurs, même signe", () => {
    const frames = RARITIES.map((r) => render("shield", "control", r));
    expect(new Set(frames).size).toBe(5);
    RARITIES.forEach((r, i) => expect(frames[i]).toContain(RARITY_EDGE[r]));
    const shapes = new Set(frames.map((h) => h.match(/<(circle|rect|polygon)/)![1] + (h.match(/points="([^"]+)"/)?.[1].split(" ").length ?? "")));
    expect(shapes.size).toBe(5);
    const sign = (h: string) => h.slice(h.indexOf("<g "));
    expect(new Set(frames.map(sign)).size).toBe(1);
  });

  it("la famille ne change que le fond", () => {
    expect(new Set(FAMILIES.map((f) => render("snowflake", f))).size).toBe(FAMILIES.length);
    for (const f of FAMILIES) expect(render("snowflake", f)).toContain(FAMILY_FLAT[f]);
  });

  it("un verbe inconnu garde une icône (point d'interrogation)", () => {
    expect(render("nope")).toContain("<path");
  });

  it("le signe ressort sur le fond de chaque famille (contraste WCAG ≥ 3)", () => {
    for (const f of FAMILIES) {
      expect(contrast(INK_COLORS.m, FAMILY_FLAT[f]), `crème/${f}`).toBeGreaterThanOrEqual(3);
      expect(contrast(INK_COLORS.a, FAMILY_FLAT[f]), `or/${f}`).toBeGreaterThanOrEqual(2.2);
    }
  });

  it("le contour de chaque rareté se détache du fond (contraste ≥ 1,5, la forme le dit aussi)", () => {
    for (const f of FAMILIES) for (const r of RARITIES) expect(contrast(RARITY_EDGE[r], FAMILY_FLAT[f]), `${r}/${f}`).toBeGreaterThanOrEqual(1.5);
  });
});
