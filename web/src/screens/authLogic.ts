// Validation du formulaire de connexion / inscription (logique pure).
import { t } from "../i18n";

export const USERNAME_RE = /^[A-Za-z0-9_]{3,16}$/;
export const PASSWORD_MIN = 8;
export const PASSWORD_MAX = 128;

export type AuthMode = "login" | "register";

export function validateUsername(value: string): string | null {
  if (!value) return t("auth.v_username_required");
  if (value.length < 3) return t("auth.v_username_min");
  if (value.length > 16) return t("auth.v_username_max");
  if (!USERNAME_RE.test(value)) return t("auth.v_username_chars");
  return null;
}

export function validatePassword(value: string, mode: AuthMode): string | null {
  if (!value) return t("auth.v_password_required");
  // À la connexion on n'impose pas les règles d'inscription : le serveur tranche.
  if (mode === "login") return null;
  if (value.length < PASSWORD_MIN) return t("auth.v_password_min", { min: PASSWORD_MIN });
  if (value.length > PASSWORD_MAX) return t("auth.v_password_max", { max: PASSWORD_MAX });
  return null;
}

export function validateConfirm(password: string, confirm: string): string | null {
  if (!confirm) return t("auth.v_confirm_required");
  if (confirm !== password) return t("auth.v_confirm_mismatch");
  return null;
}

export interface FormErrors {
  username: string | null;
  password: string | null;
  confirm: string | null;
}

export function validateForm(mode: AuthMode, v: { username: string; password: string; confirm: string }): FormErrors {
  return {
    username: mode === "register" ? validateUsername(v.username) : v.username ? null : t("auth.v_login_username_required"),
    password: validatePassword(v.password, mode),
    confirm: mode === "register" ? validateConfirm(v.password, v.confirm) : null,
  };
}

export function hasErrors(e: FormErrors): boolean {
  return !!(e.username || e.password || e.confirm);
}

/** Robustesse 0..4 (0 = vide). Sous 8 caractères, plafonnée à 1. */
export function passwordStrength(pw: string): 0 | 1 | 2 | 3 | 4 {
  if (!pw) return 0;
  const classes = [/[a-z]/, /[A-Z]/, /\d/, /[^A-Za-z0-9]/].filter((re) => re.test(pw)).length;
  let score = 0;
  if (pw.length >= PASSWORD_MIN) score++;
  if (pw.length >= 12) score++;
  if (classes >= 2) score++;
  if (classes >= 3 && pw.length >= 10) score++;
  if (pw.length < PASSWORD_MIN) score = 1;
  return Math.min(4, Math.max(1, score)) as 1 | 2 | 3 | 4;
}

/** Libellé de robustesse traduit (chaîne vide pour 0 = vide). */
export function strengthLabel(level: 0 | 1 | 2 | 3 | 4): string {
  return level ? t(`auth.strength_${level}`) : "";
}

export type ServerField = "username" | "password" | "form";

export interface MappedAuthError {
  field: ServerField;
  message: string;
}

/** Traduit le code d'erreur serveur en message rattaché à un champ. */
export function mapAuthError(code: string): MappedAuthError {
  switch (code) {
    case "username_taken":
      return { field: "username", message: t("auth.e_username_taken") };
    case "invalid_username":
      return { field: "username", message: t("auth.e_invalid_username") };
    case "weak_password":
      return { field: "password", message: t("auth.e_weak_password") };
    case "bad_credentials":
      return { field: "form", message: t("auth.e_bad_credentials") };
    case "too_many_attempts":
      return { field: "form", message: t("auth.e_too_many_attempts") };
    case "network":
      return { field: "form", message: t("auth.e_network") };
    default:
      return { field: "form", message: t("auth.e_default") };
  }
}

// ---- Code de récupération (docs/spec-v2.md §1) ----

/** 4 groupes de 5 symboles, sans tirets : ce que le serveur compare. */
export const RECOVERY_CODE_LENGTH = 20;
const RECOVERY_GROUP = 5;

/** Majuscules, sans tirets ni espaces : la forme que le serveur hache. */
export function normalizeRecoveryCode(value: string): string {
  return value.toUpperCase().replace(/[^A-Z0-9]/g, "");
}

/** Remet un code saisi ou collé en `XXXXX-XXXXX-XXXXX-XXXXX` (tronqué à 20 symboles). */
export function formatRecoveryCode(value: string): string {
  const groups = normalizeRecoveryCode(value).slice(0, RECOVERY_CODE_LENGTH).match(/.{1,5}/g);
  return groups ? groups.join("-") : "";
}

export function validateRecoveryCode(value: string): string | null {
  const n = normalizeRecoveryCode(value);
  if (!n) return t("auth.v_code_required");
  if (n.length !== RECOVERY_CODE_LENGTH) return t("auth.v_code_length", { length: RECOVERY_CODE_LENGTH, group: RECOVERY_GROUP });
  return null;
}

export interface RecoverErrors {
  username: string | null;
  code: string | null;
  password: string | null;
  confirm: string | null;
}

export function validateRecovery(v: { username: string; code: string; password: string; confirm: string }): RecoverErrors {
  return {
    username: v.username ? null : t("auth.v_login_username_required"),
    code: validateRecoveryCode(v.code),
    // Le nouveau mot de passe suit les règles d'inscription.
    password: validatePassword(v.password, "register"),
    confirm: validateConfirm(v.password, v.confirm),
  };
}

export function hasRecoverErrors(e: RecoverErrors): boolean {
  return !!(e.username || e.code || e.password || e.confirm);
}

/** Traduit une erreur des routes de récupération (mot de passe oublié, génération du code). */
export function mapRecoveryError(code: string): MappedAuthError {
  switch (code) {
    case "bad_recovery":
      return { field: "form", message: t("auth.e_bad_recovery") };
    case "bad_credentials":
      return { field: "password", message: t("auth.e_bad_password") };
    case "too_many_attempts":
      return { field: "form", message: t("auth.e_too_many_tries") };
    case "weak_password":
      return { field: "password", message: t("auth.e_weak_password") };
    case "unauthorized":
      return { field: "form", message: t("auth.e_unauthorized") };
    default:
      return mapAuthError(code);
  }
}
