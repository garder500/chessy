// Code de récupération du compte (docs/spec-v2.md §1) : affichage unique avec copie et
// confirmation, formulaire « Mot de passe oublié ? » et génération depuis les réglages.
import { useId, useState } from "react";
import type { FormEvent } from "react";
import { api, ApiError } from "../api";
import { useT } from "../i18n";
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
  const t = useT();
  const uid = useId();
  const [saved, setSaved] = useState(false);
  const [copy, setCopy] = useState<"idle" | "done" | "failed">("idle");

  async function onCopy() {
    setCopy((await copyText(code)) ? "done" : "failed");
  }

  return (
    <div className="au-form rc-box">
      <p className="rc-intro">{intro}</p>
      <p className="rc-code mono" aria-label={t("recovery.code_label")}>
        {code}
      </p>
      <div className="rc-copy">
        <button type="button" className="btn sm" onClick={onCopy} autoFocus>
          {t("recovery.copy")}
        </button>
        <span className="rc-copy-msg" aria-live="polite">
          {copy === "done" && t("recovery.copied")}
          {copy === "failed" && t("recovery.copy_failed")}
        </span>
      </div>
      <p className="au-hint">{t("recovery.warning")}</p>
      <label className="rc-saved" htmlFor={`${uid}-saved`}>
        <input id={`${uid}-saved`} type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        <span>{t("recovery.saved")}</span>
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
  const t = useT();
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
      <p className="au-hint">{t("recovery.forgot_intro")}</p>
      <div className="au-field">
        <label className="field-label" htmlFor={id("username")}>
          {t("auth.username")}
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
          {t("recovery.code_label")}
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
          {t("recovery.new_password")}
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
            aria-label={showPw ? t("auth.hide_pw_label") : t("auth.show_pw_label")}
          >
            {showPw ? t("auth.hide") : t("auth.show")}
          </button>
        </div>
        {passwordErr && <p className="field-msg">{passwordErr}</p>}
      </div>
      <div className="au-field">
        <label className="field-label" htmlFor={id("confirm")}>
          {t("auth.confirmation")}
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
        {loading ? t("recovery.verifying") : t("recovery.reset")}
      </button>
      <p className="au-switch">
        <button type="button" className="link" onClick={onBack} disabled={loading}>
          {t("recovery.back")}
        </button>
      </p>
    </form>
  );
}

/** Réglages du profil : (re)générer le code de récupération, après avoir confirmé le mot de passe. */
export function RecoveryCodeSettings() {
  const t = useT();
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
      setError(t("recovery.current_password_required"));
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
      <h2 id={`${uid}-rc-h`} className="st-h">{t("recovery.code_label")}</h2>
      <p className="muted st-hint rc-settings-p">{t("recovery.settings_hint")}</p>
      <button type="button" className="btn block" onClick={() => setOpen(true)}>
        {t("recovery.generate_btn")}
      </button>
      {/* Une fois le code généré, l'ancien ne marche plus : le panneau ne se ferme qu'avec « Terminé ». */}
      <Sheet open={open} title={t("recovery.code_label")} onClose={code ? () => undefined : close}>
        {code ? (
          <RecoveryCodeBox
            code={code}
            intro={t("recovery.new_code_intro")}
            doneLabel={t("recovery.done")}
            onDone={close}
          />
        ) : (
          <form className="au-form" onSubmit={onSubmit} noValidate>
            <div className="au-field">
              <label className="field-label" htmlFor={`${uid}-password`}>
                {t("recovery.current_password")}
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
              {loading ? t("recovery.generating") : t("recovery.generate")}
            </button>
          </form>
        )}
      </Sheet>
    </section>
  );
}
