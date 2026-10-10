import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, beforeEach, describe, expect, it } from "vitest";
import { skillEntry } from "./catalog";
import { ambientEffects } from "./game/logic";
import { setLang } from "./i18n";
import { forgedDef, isForgedId, loadForged, noticeForged, rememberForged, resetForged, unknownForged, type ForgedDef } from "./forged";
import { NO_PIECE } from "./protocol";
import { SkillArt, SkillSprite } from "./ui/SkillArt";
import { GLYPH_NAMES, RARITY_EDGE, SCENES } from "./ui/forgedIcon";

/** Les glyphes que le serveur peut nommer (`GLYPHS` dans crates/chessy-engine/src/forge/identity.rs). */
const SERVER_GLYPHS = [
  "snowflake", "shield", "veil", "morph", "crown", "erase", "banner", "portal", "echo", "swap", "summon", "ankh", "dove", "mirror", "fog", "mute", "domain",
];

const def = (n: number, over: Partial<ForgedDef> = {}): ForgedDef => ({
  id: `forged_${n}`,
  name: `Givre d'Alfen ${n}`,
  description: "Immobilise une pièce ennemie.",
  family: "control",
  rarity: "rare",
  unique: false,
  redundant: false,
  max_uses: 1,
  icon: { glyph: "snowflake", piece: "knight", badge: "short" },
  sound: { effect: 0, degree: 2, timbre: 1, length: 1 },
  ...over,
});

const reply = (skills: ForgedDef[]) => async () => ({ ok: true, json: async () => ({ skills }) });

beforeEach(() => resetForged());

describe("identifiants forgés", () => {
  it("ne reconnaît que forged_<n>", () => {
    expect(isForgedId("forged_12")).toBe(true);
    for (const bad of ["forged_", "forged_x", "freeze", "xforged_1", "forged_1x"]) expect(isForgedId(bad), bad).toBe(false);
  });

  it("repère les identifiants inconnus dans un message", () => {
    const text = JSON.stringify({ type: "deck_update", deck: ["freeze", "forged_5", "forged_9"], gained: "forged_9", lost: null });
    expect(unknownForged(text).sort()).toEqual(["forged_5", "forged_9"]);
    rememberForged([def(5)]);
    expect(unknownForged(text)).toEqual(["forged_9"]);
  });
});

describe("chargement", () => {
  it("demande les définitions manquantes en une requête et les garde", async () => {
    const urls: string[] = [];
    await loadForged(["forged_5", "forged_9", "freeze"], async (u) => {
      urls.push(u);
      return { ok: true, json: async () => ({ skills: [def(5), def(9)] }) };
    });
    expect(urls).toEqual(["/api/skills/forged?ids=5,9"]);
    expect(forgedDef("forged_5")?.name).toContain("5");
    // Rien de plus à demander.
    await loadForged(["forged_5"], async () => {
      throw new Error("ne doit pas être appelé");
    });
  });

  it("n'envoie pas deux fois la même requête et attend celle en cours", async () => {
    let calls = 0;
    const slow = async () => {
      calls++;
      await new Promise((r) => setTimeout(r, 5));
      return { ok: true, json: async () => ({ skills: [def(7)] }) };
    };
    const a = loadForged(["forged_7"], slow);
    const b = loadForged(["forged_7"], slow);
    await Promise.all([a, b]);
    expect(calls).toBe(1);
    expect(forgedDef("forged_7")).toBeDefined();
  });

  it("ne redemande pas en boucle un identifiant que le serveur ne connaît pas", async () => {
    await loadForged(["forged_404"], reply([]));
    expect(unknownForged('"forged_404"')).toEqual([]);
  });

  it("réessaie après une erreur réseau, mais pas tout de suite", async () => {
    await loadForged(["forged_3"], async () => {
      throw new Error("réseau");
    });
    expect(unknownForged('"forged_3"')).toEqual([]);
    expect(unknownForged('"forged_3"', Date.now() + 6000)).toEqual(["forged_3"]);
  });

  it("noticeForged lance le chargement de ce qui manque", async () => {
    const calls: string[] = [];
    const original = globalThis.fetch;
    globalThis.fetch = (async (u: string) => {
      calls.push(u);
      return { ok: true, json: async () => ({ skills: [def(2)] }) };
    }) as unknown as typeof fetch;
    try {
      noticeForged('{"deck":["forged_2"]}');
      await new Promise((r) => setTimeout(r, 10));
    } finally {
      globalThis.fetch = original;
    }
    expect(calls).toEqual(["/api/skills/forged?ids=2"]);
    expect(forgedDef("forged_2")).toBeDefined();
  });
});

describe("fiche et icône", () => {
  beforeAll(() => setLang("fr"));
  it("la fiche d'une compétence forgée vient de sa définition, avec une fiche d'attente avant", () => {
    expect(skillEntry("forged_8").name).toBe("Pouvoir forgé");
    rememberForged([def(8, { rarity: "legendary", unique: true })]);
    const entry = skillEntry("forged_8");
    expect(entry.name).toBe("Givre d'Alfen 8");
    expect(entry.rarity).toBe("legendary");
    expect(entry.unique).toBe(true);
  });

  it("chaque glyphe que le serveur peut nommer a un dessin", () => {
    for (const g of SERVER_GLYPHS) expect(SCENES[g], g).toBeTruthy();
    expect(GLYPH_NAMES.sort()).toEqual([...SERVER_GLYPHS].sort());
  });

  it("dessine une icône forgée : la scène de l'effet sur une pièce, et le cadre de rareté", () => {
    rememberForged([def(4)]);
    const html = renderToStaticMarkup(createElement("div", null, createElement(SkillSprite), createElement(SkillArt, { id: "forged_4", size: 40 })));
    expect(html).toContain(RARITY_EDGE.rare);
    expect(html).toContain("<clipPath");
    expect(html).not.toContain("#sk-forged_4");
  });

  it("une icône sans définition reste dessinable (point d'interrogation)", () => {
    const html = renderToStaticMarkup(createElement(SkillArt, { id: "forged_99", size: 40 }));
    expect(html).toContain("<svg");
  });
});

describe("effets de partie entière", () => {
  beforeAll(() => setLang("fr"));
  const effect = (kind: "truce" | "fog" | "silenced", expires_at: number, owner?: "white" | "black") => ({ kind, piece: NO_PIECE, expires_at, owner });

  it("les liste avec la durée restante, et ignore ceux qui sont finis", () => {
    const view = { ply: 4, you: "white" as const, effects: [effect("truce", 8), effect("fog", 4), { kind: "frozen" as const, piece: 3, expires_at: 9 }] };
    const out = ambientEffects(view);
    expect(out.map((a) => [a.kind, a.turns])).toEqual([["truce", 2]]);
  });

  it("dit à qui s'adresse le silence", () => {
    const base = { ply: 0, effects: [effect("silenced", 4, "white")] };
    expect(ambientEffects({ ...base, you: "white" })[0].label).toContain("vous");
    expect(ambientEffects({ ...base, you: "black" })[0].label).toContain("l'adversaire");
  });
});
