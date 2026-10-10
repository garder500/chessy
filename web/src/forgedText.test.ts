import { beforeEach, describe, expect, it } from "vitest";
import { skillEntry } from "./catalog";
import { rememberForged, resetForged, type ForgedBricks, type ForgedDef } from "./forged";
import { ACTIONS, forgedDescription, forgedName, NOUNS_PER_ACTION } from "./forgedText";
import { dictKeys, LANGS, setLang, type Lang } from "./i18n";
import fixture from "./forgedText.fixture.json";

/** Ce que le serveur décrit (en français), tel que `crates/chessy-engine/tests/skills/forged.rs` l'écrit. */
interface Served {
  name: string;
  name_parts: { noun: number; proper: string };
  description: string;
  bricks: ForgedBricks;
}
const served = fixture as unknown as Served[];

const asDef = (s: Served, n = 1): ForgedDef => ({
  id: `forged_${n}`,
  name: s.name,
  name_parts: s.name_parts,
  description: s.description,
  family: "control",
  rarity: "rare",
  unique: false,
  redundant: false,
  max_uses: s.bricks.max_uses ?? 1,
  icon: { glyph: "snowflake" },
  sound: { effect: 0, degree: 0, timbre: 0, length: 0 },
  bricks: s.bricks,
});

beforeEach(() => resetForged());

describe("texte des compétences forgées", () => {
  it("le fixture couvre toutes les actions", () => {
    expect(new Set(served.map((s) => s.bricks.action))).toEqual(new Set(ACTIONS));
  });

  it("en français, redit exactement le nom et la description du serveur", () => {
    setLang("fr");
    for (const s of served) {
      const def = asDef(s);
      expect(forgedName(def), s.name).toBe(s.name);
      expect(forgedDescription(def), s.description).toBe(s.description);
    }
  });

  it.each(LANGS.filter((l) => l !== "fr"))("en %s, ne laisse ni clé ni accolade et ne reste pas en français", (lang: Lang) => {
    setLang(lang);
    let changed = 0;
    for (const s of served) {
      const def = asDef(s);
      const name = forgedName(def);
      const description = forgedDescription(def);
      for (const text of [name, description]) {
        expect(text, `${lang}: ${s.description}`).not.toMatch(/fgtext\.|game\.piece|\{\w+\}|undefined/);
        expect(text.length).toBeGreaterThan(0);
      }
      expect(name.endsWith(s.name_parts.proper), name).toBe(true);
      if (description !== s.description) changed++;
    }
    expect(changed).toBe(served.length);
    setLang("fr");
  });

  it("traduit un exemple dans chaque langue", () => {
    const bricks: ForgedBricks = {
      action: "morph", side: "enemy", into: "queen", kinds: ["rook"], selector_kinds: ["rook"], zone: "center",
      plies: 4, permanent: false, condition: "behind", constraints: ["forbid_mate"], max_uses: 2, free_action: true,
    };
    const s: Served = { name: "Mue d'Alfen", name_parts: { noun: 1, proper: "Alfen" }, description: "fr", bricks };
    const expected: Record<Lang, [string, string]> = {
      en: [
        "Molt of Alfen",
        "Turns an enemy piece (not the king) into a queen for 4 moves (2 per side), then it returns to its shape. Only targets: rooks. Only in the center of the board (files c to f, ranks 3 to 6). Usable only if you have less material than your opponent. Refused if it would checkmate your opponent. Does not use up your turn. Usable 2 times per game.",
      ],
      fr: [
        "Mue d'Alfen",
        "Transforme une pièce ennemie (pas le roi) en dame pendant 4 coups (2 de chaque camp), puis elle reprend sa forme. Ne vise que : tours. Seulement au centre de l'échiquier (colonnes c à f, rangées 3 à 6). Utilisable seulement si tu as moins de matériel que l'adversaire. Refusé s'il met l'adversaire échec et mat. Ne consomme pas ton tour. Utilisable 2 fois par partie.",
      ],
      de: ["Häutung von Alfen", "Verwandelt eine gegnerische Figur (nicht den König) für 4 Züge (2 pro Seite) in eine Dame, danach nimmt sie ihre Gestalt wieder an. Zielt nur auf: Türme. Nur in der Brettmitte (Linien c bis f, Reihen 3 bis 6). Nur nutzbar, wenn du weniger Material hast als der Gegner. Abgelehnt, wenn der Gegner dadurch schachmatt wäre. Verbraucht deinen Zug nicht. 2-mal pro Partie nutzbar."],
      es: ["Muda de Alfen", "Transforma una pieza enemiga (no el rey) en una dama durante 4 jugadas (2 por bando), y luego recupera su forma. Solo apunta a: torres. Solo en el centro del tablero (columnas c a f, filas 3 a 6). Solo utilizable si tienes menos material que el adversario. Rechazado si dejaría al adversario en jaque mate. No consume tu turno. Utilizable 2 veces por partida."],
      pt: ["Muda de Alfen", "Transforma uma peça inimiga (não o rei) em uma dama durante 4 lances (2 de cada lado), e depois ela volta à sua forma. Só tem como alvo: torres. Só no centro do tabuleiro (colunas c a f, fileiras 3 a 6). Só utilizável se você tiver menos material que o adversário. Recusado se deixasse o adversário em xeque-mate. Não consome o seu turno. Utilizável 2 vezes por partida."],
    };
    for (const lang of LANGS) {
      setLang(lang);
      expect([forgedName(asDef(s)), forgedDescription(asDef(s))], lang).toEqual(expected[lang]);
    }
    setLang("fr");
  });

  it("élide « de » devant une voyelle en français seulement", () => {
    const s = { name: "Givre d'Alfen", name_parts: { noun: 0, proper: "Alfen" }, bricks: { action: "freeze" } };
    setLang("fr");
    expect(forgedName(s)).toBe("Givre d'Alfen");
    setLang("en");
    expect(forgedName(s)).toBe("Frost of Alfen");
    setLang("fr");
  });

  it("garde le texte du serveur quand les briques ou les parties du nom manquent", () => {
    setLang("en");
    const old = { name: "Givre d'Alfen", description: "Immobilise une pièce ennemie." };
    expect(forgedName(old)).toBe("Givre d'Alfen");
    expect(forgedDescription(old)).toBe("Immobilise une pièce ennemie.");
    // Une métamorphose dont le serveur n'a pas dit en quoi : on ne devine pas.
    const morph = { ...old, bricks: { action: "morph", side: "own", constraints: [] } };
    expect(forgedDescription(morph)).toBe(old.description);
    // Une action que ce client ne connaît pas.
    expect(forgedDescription({ ...old, bricks: { action: "levitate", side: "own", constraints: [] } })).toBe(old.description);
    setLang("fr");
  });

  it("la fiche suit la langue courante", () => {
    rememberForged([asDef(served.find((s) => s.bricks.action === "freeze")!, 7)]);
    setLang("fr");
    const fr = skillEntry("forged_7");
    setLang("en");
    const en = skillEntry("forged_7");
    expect(en.description).not.toBe(fr.description);
    expect(en.description).toMatch(/^Immobilizes an enemy piece/);
    setLang("fr");
  });

  it("chaque langue traduit tous les noms, zones, conditions et contraintes", () => {
    const keys = [
      ...ACTIONS.flatMap((a) => Array.from({ length: NOUNS_PER_ACTION }, (_, i) => `fgtext.noun_${a}_${i}`)),
      ...["own_half", "enemy_half", "center", "wings", "rim", "light", "dark"].map((z) => `fgtext.zone_${z}`),
      ...["behind", "ahead", "early", "late", "no_queen", "wounded"].map((c) => `fgtext.cond_${c}`),
      ...["only_in_check", "forbid_mate", "forbid_check"].map((c) => `fgtext.cons_${c}`),
    ];
    for (const lang of LANGS) {
      const have = new Set(dictKeys(lang));
      expect(keys.filter((k) => !have.has(k)), lang).toEqual([]);
    }
  });
});
