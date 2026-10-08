// Code de récupération du compte (docs/spec-v2.md §1) : affichage unique avec copie et
// confirmation, formulaire « Mot de passe oublié ? » et génération depuis les réglages.
import { useId, useState } from "react";
import type { FormEvent } from "react";
import { api, ApiError } from "../api";
import type { RecoverResponse } from "../api";
import { readToken } from "../store";
import { Sheet } from "../ui/Sheet";
import {
  formatRecoveryCode,
  hasRecoverErrors,
  mapRecoveryError,
  validateRecovery,
} from "./authLogic";
import type { MappedAuthError } from "./authLogic";
import "./auth.css";

/** Copie dans le presse-papiers ; `false` si le navigateur refuse (le code reste sélectionnable). */
async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // Pas d'API presse-papiers (page non sécurisée, autorisation refusée) : repli ci-dessous.
  }
  try {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    area.remove();
    return ok;
  } catch {
    return false;
  }
}

interface BoxProps {
  code: string;
  /** Phrase d'introduction (pourquoi ce code, ce qui vient de se passer). */
  intro: string;
  /** Libellé du bouton de fin, une fois la case cochée. */
  doneLabel: string;
  onDone: () => void;
}

/** Montre le code une seule fois : copie, puis confirmation explicite qu'il est noté. */
export function RecoveryCodeBox({ code, intro, doneLabel, onDone }: BoxProps) {
  const uid = useId();
  const [saved, setSaved] = useState(false);
  const [copy, setCopy] = useState<"idle" | "done" | "failed">("idle");

  async function onCopy() {
    setCopy((await copyText(code)) ? "done" : "failed");
  }

  return (
    <div className="au-form rc-box">
      <p className="rc-intro">{intro}</p>
      <p className="rc-code mono" aria-label="Code de récupération">
        {code}
      </p>
      <div className="rc-copy">
        <button type="button" className="btn sm" onClick={onCopy} autoFocus>
          Copier le code
        </button>
        <span className="rc-copy-msg" aria-live="polite">
          {copy === "done" && "Code copié."}
          {copy === "failed" && "Copie impossible : sélectionnez le code et copiez-le à la main."}
        </span>
      </div>
      <p className="au-hint">
        Il ne sera plus jamais affiché. Avec lui, vous pourrez choisir un nouveau mot de passe si vous perdez l'ancien ; sans lui, un compte dont le mot de passe est
        oublié est perdu. Gardez-le hors de cet appareil (gestionnaire de mots de passe, papier) et ne le partagez pas.
      </p>
      <label className="rc-saved" htmlFor={`${uid}-saved`}>
        <input id={`${uid}-saved`} type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        <span>J'ai noté mon code de récupération</span>
      </label>
      <button type="button" className="btn pri block" disabled={!saved} onClick={onDone}>
        {doneLabel}
      </button>
    </div>
  );
}

interface ForgotProps {
  /** Succès : la réponse porte une nouvelle session et le nouveau code, à montrer avant de continuer. */
  onRecovered: (res: RecoverResponse) => void;
  onBack: () => void;
}

/** « Mot de passe oublié ? » : pseudo, code de récupération et nouveau mot de passe. */
export function ForgotPasswordForm({ onRecovered, onBack }: ForgotProps) {
  const uid = useId();
  const [username, setUsername] = useState("");
  const [code, setCode] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [submitted, setSubmitted] = useState(false);
  const [loading, setLoading] = useState(false);
  const [serverError, setServerError] = useState<MappedAuthError | null>(null);

  const errors = validateRecovery({ username, code, password, confirm });
  const shown = (e: string | null) => (submitted ? e : null);
  const server = (field: MappedAuthError["field"]) => (serverError?.field === field ? serverError.message : null);
  const usernameErr = shown(errors.username);
  const codeErr = shown(errors.code);
  const passwordErr = shown(errors.password) ?? server("password");
  const confirmErr = shown(errors.confirm) ?? (confirm.length > 0 ? errors.confirm : null);
  const id = (name: string) => `${uid}-${name}`;

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (loading) return;
    setSubmitted(true);
    setServerError(null);
    if (hasRecoverErrors(errors)) return;
    setLoading(true);
    try {
      const res = await api.recover({ username, recovery_code: code, new_password: password });
      onRecovered(res);
    } catch (err) {
      setServerError(mapRecoveryError(err instanceof ApiError ? err.code : "unknown"));
      setLoading(false);
    }
  }

  return (
    <form className="au-form" onSubmit={onSubmit} noValidate>
      <p className="au-hint">Saisissez le code de récupération donné à l'inscription. Toutes vos sessions seront fermées et un nouveau code vous sera remis.</p>
      <div className="au-field">
        <label className="field-label" htmlFor={id("username")}>
          Pseudo
        </label>
        <input
          id={id("username")}
          className={`input${usernameErr ? " err" : ""}`}
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          autoComplete="username"
          autoCapitalize="none"
          spellCheck={false}
          maxLength={32}
          autoFocus
          aria-invalid={!!usernameErr}
          disabled={loading}
        />
        {usernameErr && <p className="field-msg">{usernameErr}</p>}
      </div>
      <div className="au-field">
        <label className="field-label" htmlFor={id("code")}>
          Code de récupération
        </label>
        <input
          id={id("code")}
          className={`input mono${codeErr ? " err" : ""}`}
          value={code}
          onChange={(e) => setCode(formatRecoveryCode(e.target.value))}
          placeholder="XXXXX-XXXXX-XXXXX-XXXXX"
          autoComplete="off"
          autoCapitalize="characters"
          spellCheck={false}
          maxLength={23}
          aria-invalid={!!codeErr}
          disabled={loading}
        />
        {codeErr && <p className="field-msg">{codeErr}</p>}
      </div>
      <div className="au-field">
        <label className="field-label" htmlFor={id("password")}>
          Nouveau mot de passe
        </label>
        <div className="au-pw">
          <input
            id={id("password")}
            className={`input${passwordErr ? " err" : ""}`}
            type={showPw ? "text" : "password"}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="new-password"
            maxLength={160}
            aria-invalid={!!passwordErr}
            disabled={loading}
          />
          <button
            type="button"
            className="au-eye"
            onClick={() => setShowPw((s) => !s)}
            aria-pressed={showPw}
            aria-label={showPw ? "Masquer le mot de passe" : "Afficher le mot de passe"}
          >
            {showPw ? "Masquer" : "Afficher"}
          </button>
        </div>
        {passwordErr && <p className="field-msg">{passwordErr}</p>}
      </div>
      <div className="au-field">
        <label className="field-label" htmlFor={id("confirm")}>
          Confirmation
        </label>
        <input
          id={id("confirm")}
          className={`input${confirmErr ? " err" : ""}`}
          type={showPw ? "text" : "password"}
          value={confirm}
          onChange={(e) => setConfirm(e.target.value)}
          autoComplete="new-password"
          maxLength={160}
          aria-invalid={!!confirmErr}
          disabled={loading}
        />
        {confirmErr && <p className="field-msg">{confirmErr}</p>}
      </div>
      <div aria-live="assertive">
        {server("form") && (
          <p className="au-form-err" role="alert">
            {server("form")}
          </p>
        )}
      </div>
      <button type="submit" className="btn pri block" disabled={loading} aria-busy={loading}>
        {loading ? "Vérification…" : "Réinitialiser le mot de passe"}
      </button>
      <p className="au-switch">
        <button type="button" className="link" onClick={onBack} disabled={loading}>
          Retour à la connexion
        </button>
      </p>
    </form>
  );
}

/** Réglages du profil : (re)générer le code de récupération, après avoir confirmé le mot de passe. */
export function RecoveryCodeSettings() {
  const uid = useId();
  const [open, setOpen] = useState(false);
  const [password, setPassword] = useState("");
  const [code, setCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  function close() {
    setOpen(false);
    setPassword("");
    setCode(null);
    setError(null);
    setLoading(false);
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    const token = readToken();
    if (loading || !token) return;
    if (!password) {
      setError("Saisissez votre mot de passe actuel.");
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const res = await api.recoveryCode(token, password);
      setCode(res.recovery_code);
      setPassword("");
    } catch (err) {
      setError(mapRecoveryError(err instanceof ApiError ? err.code : "unknown").message);
    }
    setLoading(false);
  }

  return (
    <section className="card st-card" aria-labelledby={`${uid}-rc-h`}>
      <h2 id={`${uid}-rc-h`} className="st-h">Code de récupération</h2>
      <p className="muted st-hint rc-settings-p">
        Sans adresse e-mail, ce code est le seul moyen de retrouver votre compte si vous oubliez votre mot de passe. En générer un nouveau remplace l'ancien.
      </p>
      <button type="button" className="btn block" onClick={() => setOpen(true)}>
        Générer un code de récupération
      </button>
      {/* Une fois le code généré, l'ancien ne marche plus : le panneau ne se ferme qu'avec « Terminé ». */}
      <Sheet open={open} title="Code de récupération" onClose={code ? () => undefined : close}>
        {code ? (
          <RecoveryCodeBox
            code={code}
            intro="Voici votre nouveau code. L'ancien ne fonctionne plus."
            doneLabel="Terminé"
            onDone={close}
          />
        ) : (
          <form className="au-form" onSubmit={onSubmit} noValidate>
            <div className="au-field">
              <label className="field-label" htmlFor={`${uid}-password`}>
                Mot de passe actuel
              </label>
              <input
                id={`${uid}-password`}
                className={`input${error ? " err" : ""}`}
                type="password"
                value={password}
                onChange={(e) => {
                  setPassword(e.target.value);
                  setError(null);
                }}
                autoComplete="current-password"
                maxLength={160}
                aria-invalid={!!error}
                disabled={loading}
              />
              <div aria-live="polite">{error && <p className="field-msg">{error}</p>}</div>
            </div>
            <button type="submit" className="btn pri block" disabled={loading} aria-busy={loading}>
              {loading ? "Génération…" : "Générer le code"}
            </button>
          </form>
        )}
      </Sheet>
    </section>
  );
}
