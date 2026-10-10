import { afterEach, describe, expect, it, vi } from "vitest";
import { readTime, setTime, timeInfo, timeText, TIMES } from "./time";

function fakeStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => {
      data[k] = v;
    },
  };
}

describe("time", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("expose les trois durées (5, 15 et 30 min)", () => {
    expect(TIMES.map((t) => [t.id, t.minutes])).toEqual([
      ["short", 5],
      ["medium", 15],
      ["long", 30],
    ]);
  });

  it("readTime relit la valeur mémorisée et retombe sur « short » sinon", () => {
    vi.stubGlobal("localStorage", fakeStorage({ "chessy.time": "long" }));
    expect(readTime()).toBe("long");
    vi.stubGlobal("localStorage", fakeStorage({ "chessy.time": "nope" }));
    expect(readTime()).toBe("short");
    vi.stubGlobal("localStorage", fakeStorage());
    expect(readTime()).toBe("short");
  });

  it("readTime tolère un stockage inaccessible", () => {
    vi.stubGlobal("localStorage", {
      getItem: () => {
        throw new Error("denied");
      },
    });
    expect(readTime()).toBe("short");
  });

  it("setTime mémorise le choix et ne plante pas si le stockage échoue", () => {
    const storage = fakeStorage();
    vi.stubGlobal("localStorage", storage);
    setTime("medium");
    expect(storage.data["chessy.time"]).toBe("medium");

    vi.stubGlobal("localStorage", {
      setItem: () => {
        throw new Error("private mode");
      },
    });
    expect(() => setTime("long")).not.toThrow();
  });

  it("timeInfo et timeText", () => {
    expect(timeInfo("long").label).toBe("Longue");
    expect(timeInfo("bogus" as never)).toBe(TIMES[0]);
    expect(timeText("short")).toBe("Blitz · 5 min");
    expect(timeText("medium")).toBe("15 min");
    expect(timeText("long")).toBe("30 min");
  });
});
