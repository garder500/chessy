import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { fixtureAnalysis, fixtureRecord } from "./fixtures";
import {
  analysisByPly,
  bestMoveSentence,
  explainSentence,
  formatLoss,
  LABEL_MEANING,
  formatAccuracy,
  formatEval,
  isLabel,
  LABEL_COLOR,
  LABEL_ORDER,
  LABEL_TEXT,
  labelFromLoss,
  mistakeTotal,
  summarySentence,
} from "./labels";

const record = fixtureRecord();
const analysis = fixtureAnalysis(3, record);
const byPly = analysisByPly(analysis);

beforeAll(() => setLang("fr"));

describe("étiquettes", () => {
  it("nomme les cinq étiquettes en français", () => {
    expect(LABEL_ORDER.map((l) => LABEL_TEXT[l])).toEqual(["Meilleur coup", "Bon coup", "Imprécision", "Erreur", "Gaffe"]);
    expect(isLabel("blunder")).toBe(true);
    expect(isLabel("great")).toBe(false);
  });

  it("donne cinq couleurs distinctes, sans vert, la gaffe étant rouge", () => {
    const colors = LABEL_ORDER.map((l) => LABEL_COLOR[l]);
    expect(new Set(colors).size).toBe(5);
    for (const hex of colors) {
      const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
      // Le vert ne domine jamais.
      expect(g > r && g > b).toBe(false);
    }
    const [r, g, b] = [1, 3, 5].map((i) => parseInt(LABEL_COLOR.blunder.slice(i, i + 2), 16));
    expect(r).toBeGreaterThan(g * 2);
    expect(r).toBeGreaterThan(b * 2);
  });

  it("applique les seuils de perte du serveur", () => {
    expect(labelFromLoss(0)).toBe("best");
    expect(labelFromLoss(10)).toBe("best");
    expect(labelFromLoss(11)).toBe("good");
    expect(labelFromLoss(50)).toBe("good");
    expect(labelFromLoss(120)).toBe("inaccuracy");
    expect(labelFromLoss(300)).toBe("mistake");
    expect(labelFromLoss(301)).toBe("blunder");
  });
});

describe("formats", () => {
  it("écrit les évaluations en pions avec virgule et vrai signe moins", () => {
    expect(formatEval(80)).toBe("+0,8");
    expect(formatEval(-130)).toBe("−1,3");
    expect(formatEval(0)).toBe("0,0");
    expect(formatEval(4)).toBe("0,0");
    expect(formatEval(2000)).toBe("+mat");
    expect(formatEval(-2000)).toBe("−mat");
    expect(formatEval(1250)).toBe("+12,5");
  });

  it("arrondit la précision", () => {
    expect(formatAccuracy(87.4)).toBe("87 %");
    expect(formatAccuracy(120)).toBe("100 %");
    expect(formatAccuracy(-3)).toBe("0 %");
  });
});

describe("phrase du meilleur coup", () => {
  it("compare le coup joué au meilleur coup", () => {
    // Coup 9 : Fg5, le meilleur était Cc3.
    const sentence = bestMoveSentence(record.moves[8], byPly.get(9), "you");
    expect(sentence).toBe("Vous avez joué Fg5 ; le meilleur coup était Cc3 (+0,1).");
  });

  it("parle des blancs ou des noirs pour un spectateur", () => {
    expect(bestMoveSentence(record.moves[14], byPly.get(15), "white")).toContain("Les blancs ont joué Teleportation d1→h5");
    expect(bestMoveSentence(record.moves[14], byPly.get(15), "black")).toMatch(/^Les noirs ont joué /);
  });

  it("reconnaît le meilleur coup", () => {
    expect(bestMoveSentence(record.moves[0], byPly.get(1), "you")).toBe("Vous avez joué e4 : c'est le meilleur coup.");
  });

  it("n'invente rien sans analyse ou sans meilleur coup", () => {
    expect(bestMoveSentence(record.moves[0], undefined, "you")).toBeNull();
    const noBest = { ...byPly.get(9)!, best: null };
    expect(bestMoveSentence(record.moves[8], noBest, "you")).toBe("Vous avez joué Fg5 (−1,3).");
  });
});

describe("résumé", () => {
  it("indexe l'analyse par coup", () => {
    expect(byPly.size).toBe(16);
    expect(analysisByPly(null).size).toBe(0);
  });

  it("compte les coups par étiquette et par camp", () => {
    const total = (c: Record<string, number>) => Object.values(c).reduce((a, b) => a + b, 0);
    expect(total(analysis.summary.white) + total(analysis.summary.black)).toBe(16);
    expect(analysis.summary.white.blunder).toBe(1);
    expect(mistakeTotal(analysis.summary.white)).toBe(2);
    expect(mistakeTotal(analysis.summary.black)).toBe(0);
  });

  it("résume les erreurs avec accord au pluriel", () => {
    expect(summarySentence({ best: 4, good: 1, inaccuracy: 0, mistake: 0, blunder: 0 })).toBe("Aucune erreur");
    expect(summarySentence({ best: 0, good: 0, inaccuracy: 3, mistake: 1, blunder: 2 })).toBe("2 gaffes, 1 erreur, 3 imprécisions");
    expect(summarySentence({ best: 0, good: 0, inaccuracy: 1, mistake: 0, blunder: 1 })).toBe("1 gaffe, 1 imprécision");
  });

  it("calcule la précision comme le serveur : 100 · exp(−perte moyenne / 250)", () => {
    expect(analysis.accuracy.black).toBeGreaterThan(analysis.accuracy.white);
    expect(analysis.accuracy.white).toBeGreaterThanOrEqual(0);
    expect(analysis.accuracy.black).toBeLessThanOrEqual(100);
  });
});

describe("explications", () => {
  const move = { ply: 3, color: "white", notation: "Fg5" } as never;
  const ply = (label: string, loss_cp: number, eval_cp: number) => ({ ply: 3, eval_cp, best: null, loss_cp, label }) as never;

  it("accorde la perte en pions", () => {
    expect(formatLoss(30)).toBe("0,3 pion");
    expect(formatLoss(150)).toBe("1,5 pion");
    expect(formatLoss(240)).toBe("2,4 pions");
    expect(formatLoss(2500)).toBe("presque toute la partie");
  });

  it("explique chaque étiquette, avec les pions perdus", () => {
    expect(explainSentence(move, undefined)).toBeNull();
    expect(explainSentence(move, ply("best", 0, 40))).toContain("plus fort");
    expect(explainSentence(move, ply("good", 30, 40))).toContain("0,3 pion");
    expect(explainSentence(move, ply("inaccuracy", 90, 40))).toContain("0,9 pion");
    expect(explainSentence(move, ply("mistake", 210, -150))).toContain("2,1 pions");
    expect(explainSentence(move, ply("blunder", 520, -600))).toMatch(/^Gaffe : ce coup fait perdre 5,2 pions/);
  });

  it("signale un mat forcé du point de vue de celui qui a joué", () => {
    expect(explainSentence(move, ply("best", 0, 2000))).toContain("mat forcé");
    expect(explainSentence(move, ply("blunder", 2500, -2000))).toContain("mat forcé");
    // Les noirs jouent : une évaluation de +2000 (pour les blancs) est leur défaite.
    const black = { ply: 4, color: "black", notation: "Dh4" } as never;
    expect(explainSentence(black, ply("blunder", 2500, 2000))).toContain("mat forcé");
    expect(explainSentence(black, ply("mistake", 400, 300))).not.toContain("mat forcé");
  });

  it("a une définition pour chaque étiquette", () => {
    for (const l of LABEL_ORDER) expect(LABEL_MEANING[l].length).toBeGreaterThan(20);
  });
});
