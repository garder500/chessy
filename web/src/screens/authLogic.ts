// Validation du formulaire de connexion / inscription (logique pure).

export const USERNAME_RE = /^[A-Za-z0-9_]{3,16}$/;
export const PASSWORD_MIN = 8;
export const PASSWORD_MAX = 128;

export type AuthMode = "login" | "register";

export function validateUsername(value: string): string | null {
  if (!value) return "Saisissez un pseudo.";
  if (value.length < 3) return "Au moins 3 caractères.";
  if (value.length > 16) return "16 caractères au maximum.";
  if (!USERNAME_RE.test(value)) return "Lettres, chiffres et _ uniquement.";
  return null;
}

export function validatePassword(value: string, mode: AuthMode): string | null {
  if (!value) return "Saisissez un mot de passe.";
  // À la connexion on n'impose pas les règles d'inscription : le serveur tranche.
  if (mode === "login") return null;
  if (value.length < PASSWORD_MIN) return `Au moins ${PASSWORD_MIN} caractères.`;
  if (value.length > PASSWORD_MAX) return `${PASSWORD_MAX} caractères au maximum.`;
  return null;
}

export function validateConfirm(password: string, confirm: string): string | null {
  if (!confirm) return "Confirmez le mot de passe.";
  if (confirm !== password) return "Les mots de passe ne correspondent pas.";
  return null;
}

export interface FormErrors {
  username: string | null;
  password: string | null;
  confirm: string | null;
}

export function validateForm(mode: AuthMode, v: { username: string; password: string; confirm: string }): FormErrors {
  return {
    username: mode === "register" ? validateUsername(v.username) : v.username ? null : "Saisissez votre pseudo.",
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

export const STRENGTH_LABEL = ["", "Faible", "Moyen", "Bon", "Solide"] as const;

export type ServerField = "username" | "password" | "form";

export interface MappedAuthError {
  field: ServerField;
  message: string;
}

/** Traduit le code d'erreur serveur en message français rattaché à un champ. */
export function mapAuthError(code: string): MappedAuthError {
  switch (code) {
    case "username_taken":
      return { field: "username", message: "Ce pseudo est déjà pris." };
    case "invalid_username":
      return { field: "username", message: "Pseudo invalide : 3 à 16 caractères, lettres, chiffres ou _." };
    case "weak_password":
      return { field: "password", message: "Mot de passe trop faible : 8 à 128 caractères." };
    case "bad_credentials":
      return { field: "form", message: "Pseudo ou mot de passe incorrect." };
    case "too_many_attempts":
      return { field: "form", message: "Trop d'échecs de connexion. Réessayez dans quelques minutes." };
    case "network":
      return { field: "form", message: "Impossible de joindre le serveur. Vérifiez votre connexion." };
    default:
      return { field: "form", message: "Une erreur est survenue. Réessayez dans un instant." };
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
  if (!n) return "Saisissez votre code de récupération.";
  if (n.length !== RECOVERY_CODE_LENGTH) return `Le code compte ${RECOVERY_CODE_LENGTH} caractères (4 groupes de ${RECOVERY_GROUP}).`;
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
    username: v.username ? null : "Saisissez votre pseudo.",
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
      return { field: "form", message: "Pseudo ou code de récupération incorrect." };
    case "bad_credentials":
      return { field: "password", message: "Mot de passe incorrect." };
    case "too_many_attempts":
      return { field: "form", message: "Trop d'essais. Réessayez dans quelques minutes." };
    case "weak_password":
      return { field: "password", message: "Mot de passe trop faible : 8 à 128 caractères." };
    case "unauthorized":
      return { field: "form", message: "Votre session a expiré. Reconnectez-vous." };
    default:
      return mapAuthError(code);
  }
}
