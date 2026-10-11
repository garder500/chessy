import { describe, expect, it } from "vitest";
import { detectLang, dictKeys, LANGS, translate } from "./index";

describe("detectLang", () => {
  it("prend la première langue prise en charge, sans la région", () => {
    expect(detectLang(["pt-BR", "en"])).toBe("pt");
    expect(detectLang(["ja", "de-AT", "fr"])).toBe("de");
  });
  it("retombe sur l'anglais", () => {
    expect(detectLang(["ja", "zh-CN"])).toBe("en");
    expect(detectLang(undefined)).toBe("en");
    expect(detectLang([])).toBe("en");
  });
});

describe("translate", () => {
  it("renvoie la clé quand elle est inconnue", () => {
    expect(translate("fr", "nope.missing")).toBe("nope.missing");
  });
});

describe("couverture", () => {
  it("chaque langue traduit toutes les clés de l'anglais", () => {
    const en = new Set(dictKeys("en"));
    for (const lang of LANGS) {
      const have = new Set(dictKeys(lang));
      const missing = [...en].filter((k) => !have.has(k));
      expect(missing, `${lang}`).toEqual([]);
    }
  });
});
