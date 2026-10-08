import { describe, expect, it } from "vitest";
import {
  formatRecoveryCode,
  hasErrors,
  hasRecoverErrors,
  mapAuthError,
  mapRecoveryError,
  normalizeRecoveryCode,
  passwordStrength,
  validateConfirm,
  validateForm,
  validatePassword,
  validateRecovery,
  validateRecoveryCode,
  validateUsername,
} from "./authLogic";

describe("validateUsername", () => {
  it("accepte 3 à 16 caractères alphanumériques ou _", () => {
    expect(validateUsername("abc")).toBeNull();
    expect(validateUsername("Joueur_01")).toBeNull();
    expect(validateUsername("a".repeat(16))).toBeNull();
  });
  it("refuse le reste", () => {
    expect(validateUsername("")).not.toBeNull();
    expect(validateUsername("ab")).not.toBeNull();
    expect(validateUsername("a".repeat(17))).not.toBeNull();
    expect(validateUsername("jo seph")).not.toBeNull();
    expect(validateUsername("élan")).not.toBeNull();
    expect(validateUsername("a-b-c")).not.toBeNull();
  });
});

describe("validatePassword", () => {
  it("impose 8 à 128 caractères à l'inscription", () => {
    expect(validatePassword("1234567", "register")).not.toBeNull();
    expect(validatePassword("12345678", "register")).toBeNull();
    expect(validatePassword("x".repeat(128), "register")).toBeNull();
    expect(validatePassword("x".repeat(129), "register")).not.toBeNull();
  });
  it("n'impose que la présence à la connexion", () => {
    expect(validatePassword("", "login")).not.toBeNull();
    expect(validatePassword("court", "login")).toBeNull();
  });
});

describe("validateConfirm / validateForm", () => {
  it("compare la confirmation", () => {
    expect(validateConfirm("abcdefgh", "abcdefgh")).toBeNull();
    expect(validateConfirm("abcdefgh", "abcdefgx")).toBe("Les mots de passe ne correspondent pas.");
    expect(validateConfirm("abcdefgh", "")).not.toBeNull();
  });
  it("valide un formulaire complet", () => {
    expect(hasErrors(validateForm("register", { username: "alice", password: "motdepasse", confirm: "motdepasse" }))).toBe(false);
    expect(hasErrors(validateForm("register", { username: "al", password: "motdepasse", confirm: "motdepasse" }))).toBe(true);
    expect(hasErrors(validateForm("login", { username: "alice", password: "x", confirm: "" }))).toBe(false);
    expect(validateForm("login", { username: "", password: "x", confirm: "" }).username).not.toBeNull();
  });
});

describe("passwordStrength", () => {
  it("vaut 0 pour un champ vide et 1 sous 8 caractères", () => {
    expect(passwordStrength("")).toBe(0);
    expect(passwordStrength("aB3$xY")).toBe(1);
  });
  it("monte avec la longueur et la variété", () => {
    expect(passwordStrength("aaaaaaaa")).toBe(1);
    expect(passwordStrength("abcdefg1")).toBe(2);
    expect(passwordStrength("Abcdefg1hi")).toBe(3);
    expect(passwordStrength("correct horse battery staple")).toBeGreaterThanOrEqual(3);
    expect(passwordStrength("Tr0ub4dor&3xyz!")).toBe(4);
  });
});

describe("mapAuthError", () => {
  it("rattache chaque code à son champ", () => {
    expect(mapAuthError("username_taken")).toMatchObject({ field: "username", message: "Ce pseudo est déjà pris." });
    expect(mapAuthError("invalid_username").field).toBe("username");
    expect(mapAuthError("weak_password").field).toBe("password");
    expect(mapAuthError("bad_credentials")).toMatchObject({ field: "form", message: "Pseudo ou mot de passe incorrect." });
    expect(mapAuthError("too_many_attempts").message).toMatch(/Trop d'échecs/);
    expect(mapAuthError("network").field).toBe("form");
    expect(mapAuthError("n_importe_quoi").field).toBe("form");
  });
});

describe("code de récupération", () => {
  it("normalise et reformate un code saisi à la main", () => {
    expect(normalizeRecoveryCode(" k7qf2-m9xwb 3hnra-td8lc ")).toBe("K7QF2M9XWB3HNRATD8LC");
    expect(formatRecoveryCode("k7qf2m9xwb3hnratd8lc")).toBe("K7QF2-M9XWB-3HNRA-TD8LC");
    expect(formatRecoveryCode("k7qf2-m9")).toBe("K7QF2-M9");
    expect(formatRecoveryCode("K7QF2M9XWB3HNRATD8LCZZZ")).toBe("K7QF2-M9XWB-3HNRA-TD8LC");
    expect(formatRecoveryCode("")).toBe("");
  });
  it("valide la longueur", () => {
    expect(validateRecoveryCode("")).toMatch(/Saisissez/);
    expect(validateRecoveryCode("K7QF2-M9XWB")).toMatch(/20 caractères/);
    expect(validateRecoveryCode("k7qf2m9xwb3hnratd8lc")).toBeNull();
    expect(validateRecoveryCode("K7QF2-M9XWB-3HNRA-TD8LC")).toBeNull();
  });
  it("valide le formulaire de récupération", () => {
    const ok = { username: "alice", code: "K7QF2-M9XWB-3HNRA-TD8LC", password: "battery staple", confirm: "battery staple" };
    expect(hasRecoverErrors(validateRecovery(ok))).toBe(false);
    expect(validateRecovery({ ...ok, username: "" }).username).not.toBeNull();
    expect(validateRecovery({ ...ok, password: "court", confirm: "court" }).password).toMatch(/8 caractères/);
    expect(validateRecovery({ ...ok, confirm: "autre" }).confirm).toMatch(/ne correspondent pas/);
  });
  it("traduit les erreurs des routes de récupération", () => {
    expect(mapRecoveryError("bad_recovery")).toMatchObject({ field: "form", message: "Pseudo ou code de récupération incorrect." });
    expect(mapRecoveryError("bad_credentials").field).toBe("password");
    expect(mapRecoveryError("too_many_attempts").field).toBe("form");
    expect(mapRecoveryError("weak_password").field).toBe("password");
    expect(mapRecoveryError("network").field).toBe("form");
  });
});
