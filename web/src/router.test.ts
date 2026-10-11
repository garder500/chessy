import { describe, expect, it } from "vitest";
import { hrefFor, parseHash } from "./router";

describe("routes v4", () => {
  it("reconnaît les nouvelles pages", () => {
    expect(parseHash("#/live")).toEqual({ name: "live" });
    expect(parseHash("#/games")).toEqual({ name: "games" });
    expect(parseHash("#/watch/g-1")).toEqual({ name: "watch", param: "g-1" });
    expect(parseHash("#/replay/g-1")).toEqual({ name: "replay", param: "g-1" });
    expect(parseHash("#/replay/g-1/analyse")).toEqual({ name: "replay", param: "g-1", sub: "analyse" });
  });

  it("retombe sur une page utile sans identifiant", () => {
    expect(parseHash("#/watch")).toEqual({ name: "live" });
    expect(parseHash("#/replay")).toEqual({ name: "games" });
    expect(parseHash("#/replay/")).toEqual({ name: "games" });
  });

  it("ignore un sous-chemin inconnu", () => {
    expect(parseHash("#/replay/g-1/autre")).toEqual({ name: "replay", param: "g-1" });
  });

  it("décode les identifiants et résiste aux séquences invalides", () => {
    expect(parseHash("#/watch/a%2Fb")).toEqual({ name: "watch", param: "a/b" });
    expect(parseHash("#/replay/%E0%A4%A")).toEqual({ name: "replay", param: "%E0%A4%A" });
  });

  it("construit les liens", () => {
    expect(hrefFor({ name: "live" })).toBe("#/live");
    expect(hrefFor({ name: "games" })).toBe("#/games");
    expect(hrefFor({ name: "watch", param: "g 1" })).toBe("#/watch/g%201");
    expect(hrefFor({ name: "replay", param: "g-1" })).toBe("#/replay/g-1");
    expect(hrefFor({ name: "replay", param: "g-1", sub: "analyse" })).toBe("#/replay/g-1/analyse");
  });

  it("garde les routes existantes", () => {
    expect(parseHash("#/profile/ada")).toEqual({ name: "profile", param: "ada" });
    expect(parseHash("")).toEqual({ name: "home" });
    expect(hrefFor({ name: "profile", param: "ada" })).toBe("#/profile/ada");
    expect(hrefFor({ name: "home" })).toBe("#/");
  });
});
