import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "../i18n";
import { BASE_STEP_MS, clampIndex, initialNav, keyToNav, navReduce, shouldHandleKey, speedLabel, stepDelay } from "./nav";

beforeAll(() => setLang("fr"));

describe("navigation de replay", () => {
  it("borne l'indice", () => {
    expect(clampIndex(-3, 10)).toBe(0);
    expect(clampIndex(99, 10)).toBe(10);
    expect(clampIndex(4.6, 10)).toBe(5);
    expect(clampIndex(NaN, 10)).toBe(0);
    expect(clampIndex(3, 0)).toBe(0);
  });

  it("avance et recule sans dépasser les bornes", () => {
    let s = initialNav(2);
    s = navReduce(s, { type: "prev" });
    expect(s.index).toBe(0);
    s = navReduce(s, { type: "next" });
    s = navReduce(s, { type: "next" });
    s = navReduce(s, { type: "next" });
    expect(s.index).toBe(2);
    expect(navReduce(s, { type: "first" }).index).toBe(0);
    expect(navReduce(initialNav(7), { type: "last" }).index).toBe(7);
    expect(navReduce(initialNav(7), { type: "goto", index: 50 }).index).toBe(7);
    expect(navReduce(initialNav(7), { type: "goto", index: -1 }).index).toBe(0);
  });

  it("la navigation manuelle interrompt la lecture", () => {
    const playing = navReduce(initialNav(5), { type: "play" });
    expect(playing.playing).toBe(true);
    expect(navReduce(playing, { type: "next" }).playing).toBe(false);
    expect(navReduce(playing, { type: "goto", index: 3 }).playing).toBe(false);
  });

  it("la lecture automatique avance puis s'arrête à la fin", () => {
    let s = navReduce(initialNav(2), { type: "play" });
    s = navReduce(s, { type: "tick" });
    expect(s).toMatchObject({ index: 1, playing: true });
    s = navReduce(s, { type: "tick" });
    expect(s).toMatchObject({ index: 2, playing: false });
    expect(navReduce(s, { type: "tick" })).toBe(s);
  });

  it("relancer la lecture depuis la fin repart du début", () => {
    const end = navReduce(initialNav(4), { type: "last" });
    expect(navReduce(end, { type: "toggle" })).toMatchObject({ index: 0, playing: true });
  });

  it("ne lit pas une partie sans coup", () => {
    expect(navReduce(initialNav(0), { type: "play" }).playing).toBe(false);
  });

  it("règle la vitesse et le délai de lecture", () => {
    const s = navReduce(initialNav(3), { type: "speed", speed: 2 });
    expect(s.speed).toBe(2);
    expect(stepDelay(0.5)).toBe(BASE_STEP_MS * 2);
    expect(stepDelay(1)).toBe(BASE_STEP_MS);
    expect(stepDelay(2)).toBe(BASE_STEP_MS / 2);
    expect(speedLabel(0.5)).toBe("0,5×");
    expect(speedLabel(2)).toBe("2×");
  });

  it("repart du début avec une nouvelle longueur", () => {
    const s = navReduce(navReduce(initialNav(3), { type: "last" }), { type: "reset", max: 9 });
    expect(s).toMatchObject({ index: 0, max: 9, playing: false });
  });
});

describe("raccourcis clavier", () => {
  it("associe les touches aux actions", () => {
    expect(keyToNav("ArrowLeft")).toEqual({ type: "prev" });
    expect(keyToNav("ArrowRight")).toEqual({ type: "next" });
    expect(keyToNav("Home")).toEqual({ type: "first" });
    expect(keyToNav("End")).toEqual({ type: "last" });
    expect(keyToNav(" ")).toEqual({ type: "toggle" });
    expect(keyToNav("a")).toBeNull();
  });

  it("ignore les champs de saisie et les modificateurs", () => {
    expect(shouldHandleKey({ key: "ArrowLeft", target: { tagName: "BODY" } })).toBe(true);
    expect(shouldHandleKey({ key: "ArrowLeft", target: { tagName: "INPUT" } })).toBe(false);
    expect(shouldHandleKey({ key: "ArrowLeft", target: { tagName: "SELECT" } })).toBe(false);
    expect(shouldHandleKey({ key: "ArrowLeft", ctrlKey: true })).toBe(false);
    expect(shouldHandleKey({ key: "Home", metaKey: true })).toBe(false);
  });

  it("laisse l'espace aux boutons focalisés mais pas les flèches", () => {
    expect(shouldHandleKey({ key: " ", target: { tagName: "BUTTON" } })).toBe(false);
    expect(shouldHandleKey({ key: " ", target: { tagName: "BODY" } })).toBe(true);
    expect(shouldHandleKey({ key: "ArrowRight", target: { tagName: "BUTTON" } })).toBe(true);
  });
});
