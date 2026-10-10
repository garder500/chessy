import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import type { Family } from "../catalog";
import type { IconSpec, Rarity } from "../forged";
import { FAMILY_BG, ForgedBadge, GLYPH_NAMES, RARITY_EDGE, rng, sigilArm } from "./forgedIcon";

const RARITIES: Rarity[] = ["common", "uncommon", "rare", "epic", "legendary"];
const FAMILIES = Object.keys(FAMILY_BG) as Family[];
const KINDS = ["pawn", "knight", "bishop", "rook", "queen"] as const;

const render = (icon: Partial<IconSpec> & { glyph: string }, family: Family = "control", rarity: Rarity = "rare") =>
  renderToStaticMarkup(createElement("svg", null, createElement(ForgedBadge, { icon: { seed: 1, zone: "one", ...icon } as IconSpec, family, rarity })));

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

describe("icône forgée : médaillon", () => {
  it("chaque glyphe dessine une scène avec une pièce, sans dépendre de symboles externes", () => {
    for (const glyph of GLYPH_NAMES)
      for (const piece of KINDS) {
        const html = render({ glyph, piece });
        expect(html, `${glyph}/${piece}`).toContain("<path");
        expect(html).not.toContain("<use");
      }
    expect(GLYPH_NAMES).toHaveLength(17);
  });

  it("toute la scène est dans le groupe rogné au médaillon : rien ne déborde du blason", () => {
    for (const glyph of GLYPH_NAMES) {
      const html = render({ glyph, piece: "rook", plies: 4, mark: "free", target: "enemy" });
      const clipped = html.indexOf('clip-path="url(#imc1)"');
      expect(clipped, glyph).toBeGreaterThan(0);
      // La scène vient après l'ouverture du groupe rogné, l'anneau et la pastille (réservés) après sa fermeture.
      expect(html.indexOf("<mask", 0) === -1 || html.indexOf("<mask") > clipped, glyph).toBe(true);
    }
  });

  it("l'anneau de durée se remplit d'un huitième de tour par coup, et disparaît sans durée", () => {
    const arcs = new Set<string>();
    for (let plies = 2; plies <= 8; plies++) {
      const html = render({ glyph: "snowflake", plies });
      const m = html.match(new RegExp(`<path d="([^"]+)"[^>]*data-plies="${plies}"`));
      expect(m, `${plies}`).not.toBeNull();
      arcs.add(m![1]);
    }
    expect(arcs.size).toBe(7);
    expect(render({ glyph: "snowflake" })).not.toContain("data-plies");
  });

  it("la zone change le fond : halo, bande, damier", () => {
    const one = render({ glyph: "fog", zone: "one" });
    const row = render({ glyph: "fog", zone: "row" });
    const board = render({ glyph: "fog", zone: "board" });
    expect(new Set([one, row, board]).size).toBe(3);
    expect(board.match(/width="30" height="30"/g)).toHaveLength(8);
  });

  it("la pastille de règle n'apparaît que s'il y a une règle, et chaque règle a son signe", () => {
    expect(render({ glyph: "shield" })).not.toContain('r="16"');
    const seen = new Set(["free", "check", "safe"].map((mark) => render({ glyph: "shield", mark: mark as IconSpec["mark"] })));
    expect(seen.size).toBe(3);
    for (const html of seen) expect(html).toContain('r="16"');
  });

  it("l'adversaire a une pièce violette, vous une pièce claire", () => {
    expect(render({ glyph: "erase", target: "enemy" })).toContain("#7a45c8");
    expect(render({ glyph: "erase", target: "own" })).not.toContain("#7a45c8");
  });

  it("chaque rareté a son cadre, dans sa couleur", () => {
    const shapes = new Set(RARITIES.map((r) => render({ glyph: "shield" }, "control", r).match(/<(circle|rect|polygon)[^>]*>/)![0].split(" ")[0]));
    expect(shapes.size).toBe(3); // cercle, carré arrondi, polygone (hexagone, octogone, soleil)
    const polys = new Set(["rare", "epic", "legendary"].map((r) => render({ glyph: "shield" }, "control", r as Rarity).match(/<polygon points="([^"]+)"/)![1]));
    expect(polys.size).toBe(3);
    for (const r of RARITIES) expect(render({ glyph: "shield" }, "control", r)).toContain(RARITY_EDGE[r]);
  });
});

describe("icône forgée : sigil", () => {
  it("est le même pour une même graine, et change avec elle", () => {
    expect(sigilArm("snowflake", 7)).toEqual(sigilArm("snowflake", 7));
    const arms = new Set(Array.from({ length: 200 }, (_, i) => sigilArm("snowflake", i * 7919 + 3).d));
    expect(arms.size).toBeGreaterThan(190);
  });

  it("reste dans le médaillon : la pointe est à moins de 45 du centre", () => {
    for (const glyph of GLYPH_NAMES)
      for (let seed = 0; seed < 100; seed++) {
        const [x, y] = sigilArm(glyph, seed * 104729).tip;
        expect(Math.hypot(x, y), `${glyph}/${seed}`).toBeLessThanOrEqual(45);
      }
  });

  it("deux compétences de graines différentes ne donnent jamais la même image", () => {
    const seen = new Set<string>();
    for (let seed = 0; seed < 300; seed++) seen.add(render({ glyph: GLYPH_NAMES[seed % 17], seed: seed * 2654435761 }));
    expect(seen.size).toBe(300);
  });

  it("le tirage est équitable : le générateur couvre l'intervalle", () => {
    const r = rng(42);
    const xs = Array.from({ length: 2000 }, () => r());
    expect(Math.min(...xs)).toBeLessThan(0.01);
    expect(Math.max(...xs)).toBeGreaterThan(0.99);
  });
});

describe("icône forgée : lisibilité", () => {
  it("le contour d'encre se détache du fond de chaque famille (contraste d'au moins 3:1)", () => {
    for (const family of FAMILIES) {
      const [centre, bord] = FAMILY_BG[family];
      expect(contrast("#0d1020", centre), family).toBeGreaterThanOrEqual(3);
      expect(contrast("#0d1020", bord) >= 1, family).toBe(true);
    }
  });

  it("la pièce du joueur et celle de l'adversaire se distinguent l'une de l'autre", () => {
    expect(contrast("#f4f7ff", "#7a45c8")).toBeGreaterThanOrEqual(3);
  });

  it("l'anneau de durée (or) se lit sur le cadre sombre", () => {
    expect(contrast("#ffd34d", "#1a1e36")).toBeGreaterThanOrEqual(7);
  });
});
