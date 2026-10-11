import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "./i18n";
import { SOLO_DEFAULT, SOLO_KEY, SOLO_MAX, SOLO_MIN, SOLO_STEP, SOLO_TIERS, clampElo, readSolo, soloTier, writeSolo } from "./solo";

beforeAll(() => setLang("fr"));

function memory(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => {
      data[k] = v;
    },
  };
}

describe("soloTier", () => {
  it.each([
    [400, "Débutant"],
    [799, "Débutant"],
    [800, "Amateur"],
    [1199, "Amateur"],
    [1200, "Club"],
    [1599, "Club"],
    [1600, "Expert"],
    [1999, "Expert"],
    [2000, "Maître"],
    [2399, "Maître"],
    [2400, "Grand Maître"],
    [2800, "Grand Maître"],
  ])("%i -> %s", (elo, name) => {
    expect(soloTier(elo).name).toBe(name);
  });

  it("covers the whole slider range with six tiers", () => {
    expect(SOLO_TIERS).toHaveLength(6);
    expect(SOLO_TIERS[0].min).toBe(SOLO_MIN);
  });
});

describe("slider bounds", () => {
  it("uses 400..2800 in steps of 50", () => {
    expect([SOLO_MIN, SOLO_MAX, SOLO_STEP]).toEqual([400, 2800, 50]);
    expect((SOLO_MAX - SOLO_MIN) % SOLO_STEP).toBe(0);
  });

  it("clamps and snaps to the grid", () => {
    expect(clampElo(0)).toBe(400);
    expect(clampElo(399)).toBe(400);
    expect(clampElo(5000)).toBe(2800);
    expect(clampElo(1224)).toBe(1200);
    expect(clampElo(1225)).toBe(1250);
    expect(clampElo(1400)).toBe(1400);
    expect(clampElo(Number.NaN)).toBe(SOLO_DEFAULT.elo);
  });
});

describe("remembered setting", () => {
  it("falls back to the default when nothing is stored", () => {
    expect(readSolo(memory())).toEqual(SOLO_DEFAULT);
    expect(readSolo(null)).toEqual(SOLO_DEFAULT);
  });

  it("round-trips through storage", () => {
    const store = memory();
    writeSolo({ elo: 1850, color: "black" }, store);
    expect(readSolo(store)).toEqual({ elo: 1850, color: "black" });
  });

  it("repairs corrupted values", () => {
    expect(readSolo(memory({ [SOLO_KEY]: "not json" }))).toEqual(SOLO_DEFAULT);
    expect(readSolo(memory({ [SOLO_KEY]: JSON.stringify({ elo: 99999, color: "green" }) }))).toEqual({ elo: 2800, color: "random" });
  });

  it("survives a storage that throws", () => {
    const broken = {
      getItem: () => {
        throw new Error("denied");
      },
      setItem: () => {
        throw new Error("denied");
      },
    };
    expect(readSolo(broken)).toEqual(SOLO_DEFAULT);
    expect(() => writeSolo(SOLO_DEFAULT, broken)).not.toThrow();
  });
});
