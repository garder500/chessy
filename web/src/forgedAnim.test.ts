import { describe, expect, it } from "vitest";
import { durationStyle } from "./forged";

describe("durationStyle", () => {
  it("lit la durée des briques comme les badges d'icônes", () => {
    expect(durationStyle({ plies: 2 })).toBe("short");
    expect(durationStyle({ plies: 5 })).toBe("long");
    expect(durationStyle({ permanent: true })).toBe("forever");
    expect(durationStyle({})).toBe("none");
    expect(durationStyle({ plies: null })).toBe("none");
  });
});
