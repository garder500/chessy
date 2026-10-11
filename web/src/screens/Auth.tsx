import { lazy, Suspense, useId, useState } from "react";
import type { FormEvent } from "react";
import { api, ApiError } from "../api";
import { useT } from "../i18n";
import { navigate } from "../router";
import { Beam } from "../ui/Beam";
import { HeroPiece } from "../ui/HeroPiece";

// three.js ne se charge qu'à la demande ; la silhouette SVG sert d'attente.
const HeroPiece3D = lazy(() => import("../ui/HeroPiece3D"));
import { Sheet } from "../ui/Sheet";
import { readToken, store, useAppState } from "../store";
import {
  hasErrors,
  mapAuthError,
  passwordStrength,
  strengthLabel,
  validateForm,
} from "./authLogic";
import type { AuthMode, FormErrors, ServerField } from "./authLogic";
import { ForgotPasswordForm, RecoveryCodeBox } from "./RecoveryCode";
import "./auth.css";

type Touched = Record<"username" | "password" | "confirm", boolean>;
const NOT_TOUCHED: Touched = { username: false, password: false, confirm: false };

const WELCOME_KEY = "chessy.welcomed";

/** Marque l'écran de bienvenue comme vu : l'application ouvre ensuite directement sur Jouer. */
export function markWelcomed() {
  try {
    localStorage.setItem(WELCOME_KEY, "1");
  } catch {
    // Mode privé : l'écran reviendra au prochain chargement.
  }
}

export function needsWelcome(): boolean {
  try {
    return !readToken() && !localStorage.getItem(WELCOME_KEY);
  } catch {
    return false;
  }
}

const ARGUMENTS = [
  { key: "auth.arg_duration", color: "var(--accent)" },
  { key: "auth.arg_skills", color: "var(--rar-rare)" },
  { key: "auth.arg_ranked", color: "var(--rar-legendary)" },
];

export function Auth() {
  const t = useT();
  const uid = useId();
  const { account } = useAppState();
  const [sheet, setSheet] = useState<AuthMode | null>(null);
  // Le dernier mode choisi reste actif pendant la fermeture du panneau.
  const [lastMode, setLastMode] = useState<AuthMode>("register");
  const mode: AuthMode = sheet ?? lastMode;
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [showPw, setShowPw] = useState(false);
  const [touched, setTouched] = useState<Touched>(NOT_TOUCHED);
  const [submitted, setSubmitted] = useState(false);
  const [loading, setLoading] = useState(false);
  const [serverError, setServerError] = useState<{ field: ServerField; message: string } | null>(null);
  // « Mot de passe oublié ? » : le panneau de connexion montre le formulaire de récupération.
  const [recovering, setRecovering] = useState(false);
  // Session et code de récupération à montrer une fois, avant d'entrer dans l'application :
  // la session n'est appliquée (donc l'écran quitté) qu'après « J'ai noté mon code ».
  const [issued, setIssued] = useState<{ token: string; code: string; why: "register" | "recover" } | null>(null);

  const errors: FormErrors = validateForm(mode, { username, password, confirm });
  const strength = passwordStrength(password);
  const register = mode === "register";

  // Une erreur est visible une fois le champ quitté, ou après une tentative d'envoi.
  // La confirmation réagit en direct dès qu'elle contient quelque chose.
  const visible = (f: keyof Touched): string | null => {
    const shown = touched[f] || submitted || (f === "confirm" && confirm.length > 0);
    return shown ? errors[f] : null;
  };
  const usernameErr = visible("username") ?? (serverError?.field === "username" ? serverError.message : null);
  const passwordErr = visible("password") ?? (serverError?.field === "password" ? serverError.message : null);
  const confirmErr = register ? visible("confirm") : null;

  const touch = (f: keyof Touched) => setTouched((prev) => (prev[f] ? prev : { ...prev, [f]: true }));
  const clearServer = () => setServerError(null);

  function switchMode(next: AuthMode) {
    if (next === mode && sheet) return;
    setSheet(next);
    setLastMode(next);
    setTouched(NOT_TOUCHED);
    setSubmitted(false);
    setServerError(null);
    setConfirm("");
    setRecovering(false);
  }

  /** Entre dans l'application avec la session émise (inscription ou récupération). */
  function enter(token: string) {
    store.applyAuth(token);
    markWelcomed();
    navigate({ name: "home" });
  }

  async function onSubmit(e: FormEvent) {
    e.preventDefault();
    if (loading) return;
    setSubmitted(true);
    setServerError(null);
    if (hasErrors(errors)) return;
    setLoading(true);
    try {
      let res;
      if (register) {
        // Un jeton d'invité permet de promouvoir l'invité en gardant son deck.
        const stored = readToken();
        const guestToken = stored && (!account || account.guest) ? stored : undefined;
        res = await api.register({ username, password, ...(guestToken ? { guest_token: guestToken } : {}) });
      } else {
        res = await api.login({ username, password });
      }
      if (res.recovery_code) {
        // Le code n'est montré qu'ici : on attend la confirmation avant d'appliquer la session.
        setIssued({ token: res.token, code: res.recovery_code, why: "register" });
        setPassword("");
        setConfirm("");
        setLoading(false);
        return;
      }
      enter(res.token);
    } catch (err) {
      setServerError(mapAuthError(err instanceof ApiError ? err.code : "unknown"));
      setLoading(false);
    }
  }

  const id = (name: string) => `${uid}-${name}`;
  const formError = serverError?.field === "form" ? serverError.message : null;

  const closeSheet = () => {
    setSheet(null);
    setTouched(NOT_TOUCHED);
    setSubmitted(false);
    setServerError(null);
    setRecovering(false);
  };
  const title = issued
    ? t("auth.title_recovery_code")
    : recovering && !register
      ? t("auth.title_forgot")
      : register
        ? t("auth.title_register")
        : t("auth.title_login");

  return (
    <main className="wl">
      <Beam width={620} height={540} />
      <div className="wl-mid">
        <Suspense fallback={<HeroPiece kind="king" className="wl-piece" />}>
          <HeroPiece3D kind="king" className="wl-piece" />
        </Suspense>
        <h1 id={id("title")} className="wl-title">
          Chessy
        </h1>
        <p className="wl-lead">{t("auth.lead")}</p>
        <ul className="wl-args">
          {ARGUMENTS.map((a) => (
            <li key={a.key}>
              <span className="hex wl-hex" style={{ background: a.color }} aria-hidden="true" />
              {t(a.key)}
            </li>
          ))}
        </ul>
      </div>
      <div className="wl-act">
        <button type="button" className="btn pri block" onClick={() => switchMode("register")}>
          {t("auth.title_register")}
        </button>
        <button type="button" className="btn block" onClick={() => switchMode("login")}>
          {t("auth.title_login")}
        </button>
        <button
          type="button"
          className="link wl-guest"
          onClick={() => {
            markWelcomed();
            navigate({ name: "home" });
          }}
        >
          {t("auth.guest")}
        </button>
      </div>

      {/* Pendant l'affichage du code, le panneau ne se ferme pas : le code ne se montre qu'une fois. */}
      <Sheet open={sheet !== null} title={title} onClose={issued ? () => undefined : closeSheet}>
        {issued && (
          <RecoveryCodeBox
            code={issued.code}
            intro={
              issued.why === "register"
                ? t("auth.intro_register")
                : t("auth.intro_recover")
            }
            doneLabel={t("auth.continue")}
            onDone={() => enter(issued.token)}
          />
        )}
        {!issued && recovering && !register && (
          <ForgotPasswordForm
            onRecovered={(res) => setIssued({ token: res.token, code: res.recovery_code, why: "recover" })}
            onBack={() => setRecovering(false)}
          />
        )}
        {/* Masqué (et non démonté) pendant les autres vues : la saisie reste là au retour. */}
        <form className="au-form" onSubmit={onSubmit} noValidate hidden={!!issued || (recovering && !register)}>
          <div className="au-field">
            <label className="field-label" htmlFor={id("username")}>
              {t("auth.username")}
            </label>
            <input
              id={id("username")}
              className={`input${usernameErr ? " err" : ""}`}
              value={username}
              onChange={(e) => {
                setUsername(e.target.value);
                clearServer();
              }}
              onBlur={() => touch("username")}
              autoComplete="username"
              autoCapitalize="none"
              spellCheck={false}
              maxLength={32}
              aria-invalid={!!usernameErr}
              aria-describedby={id("username-msg")}
              disabled={loading}
            />
            <div id={id("username-msg")} aria-live="polite">
              {usernameErr ? (
                <p className="field-msg">{usernameErr}</p>
              ) : register ? (
                <p className="au-hint">{t("auth.username_hint")}</p>
              ) : null}
            </div>
          </div>

          <div className="au-field">
            <label className="field-label" htmlFor={id("password")}>
              {t("auth.password")}
            </label>
            <div className="au-pw">
              <input
                id={id("password")}
                className={`input${passwordErr ? " err" : ""}`}
                type={showPw ? "text" : "password"}
                value={password}
                onChange={(e) => {
                  setPassword(e.target.value);
                  clearServer();
                }}
                onBlur={() => touch("password")}
                autoComplete={register ? "new-password" : "current-password"}
                maxLength={160}
                aria-invalid={!!passwordErr}
                aria-describedby={id("password-msg")}
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
            <div id={id("password-msg")} aria-live="polite">
              {passwordErr && <p className="field-msg">{passwordErr}</p>}
            </div>
            {!register && (
              <button type="button" className="link au-forgot" onClick={() => setRecovering(true)} disabled={loading}>
                {t("auth.forgot")}
              </button>
            )}
            {register && (
              <div className="au-meter" data-level={strength}>
                <div className="au-meter-bars" role="img" aria-label={t("auth.strength_aria", { label: strengthLabel(strength) || t("auth.strength_empty") })}>
                  {[1, 2, 3, 4].map((n) => (
                    <span key={n} className={n <= strength ? "on" : ""} />
                  ))}
                </div>
                <span className="au-meter-label">{strengthLabel(strength) || t("auth.strength_min")}</span>
              </div>
            )}
          </div>

          {register && (
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
                onBlur={() => touch("confirm")}
                autoComplete="new-password"
                maxLength={160}
                aria-invalid={!!confirmErr}
                aria-describedby={id("confirm-msg")}
                disabled={loading}
              />
              <div id={id("confirm-msg")} aria-live="polite">
                {confirmErr && <p className="field-msg">{confirmErr}</p>}
              </div>
            </div>
          )}

          <div aria-live="assertive">
            {formError && (
              <p className="au-form-err" role="alert">
                {formError}
              </p>
            )}
          </div>

          <button type="submit" className="btn pri block" disabled={loading} aria-busy={loading}>
            {loading ? (register ? t("auth.creating") : t("auth.logging_in")) : title}
          </button>
          <p className="au-switch">
            {register ? t("auth.have_account") : t("auth.no_account")}{" "}
            <button type="button" className="link" onClick={() => switchMode(register ? "login" : "register")} disabled={loading}>
              {register ? t("auth.title_login") : t("auth.title_register")}
            </button>
          </p>
        </form>
      </Sheet>
    </main>
  );
}
