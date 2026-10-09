import { useEffect, useSyncExternalStore, type CSSProperties } from "react";
import { LANGS, LANG_NAMES, getLang, hasChosenLang, setLang, useLang, useT, type Lang } from "../i18n";
import { CATALOG, FAMILY_LABEL } from "../catalog";
import { sfx, skillSfx, type SfxName } from "../sound";
import { store, useAppState } from "../store";
import { ACCENTS, BOARD_THEMES, PIECE_SETS, setTheme, useTheme, type ColorMode, type MoveMode } from "../theme";
import { BoardPreview } from "../ui/BoardPreview";
import { SkillArt } from "../ui/SkillArt";
import "../ui/moderation.css";
import "./settings.css";

function useSoundSettings() {
  return useSyncExternalStore(sfx.subscribe, sfx.getSettings);
}

function Switch({ checked, onChange, label, hint }: { checked: boolean; onChange: (v: boolean) => void; label: string; hint?: string }) {
  return (
    <div className="st-row">
      <div className="st-row-txt">
        <span className="st-label">{label}</span>
        {hint && <span className="st-hint muted">{hint}</span>}
      </div>
      <button type="button" role="switch" aria-checked={checked} aria-label={label} className="st-switch" data-sfx="off" onClick={() => onChange(!checked)}>
        <span />
      </button>
    </div>
  );
}

function Slider({ value, onChange, label, disabled }: { value: number; onChange: (v: number) => void; label: string; disabled?: boolean }) {
  return (
    <label className="st-row st-slider">
      <span className="st-label">{label}</span>
      <input
        type="range"
        min={0}
        max={100}
        step={1}
        value={Math.round(value * 100)}
        disabled={disabled}
        onChange={(e) => onChange(Number(e.target.value) / 100)}
        aria-valuetext={`${Math.round(value * 100)} %`}
      />
      <span className="mono st-val">{Math.round(value * 100)}</span>
    </label>
  );
}

const COLOR_MODES: ColorMode[] = ["system", "light", "dark"];

/** Aperçu d'un son : ignore les interrupteurs de catégorie, jamais le réglage général. */
const preview = (name: SfxName) => sfx.play(name, { force: true });

const GROUPS: { id: string; items: SfxName[] }[] = [
  { id: "moves", items: ["move", "capture", "castle", "promote", "check", "illegal"] },
  { id: "game", items: ["game_start", "your_turn", "low_time", "game_win", "game_lose", "game_draw"] },
  { id: "effects", items: ["trap_sprung", "shield", "pushed", "saved", "vanish"] },
  { id: "social", items: ["match_found", "chat", "friend_request", "challenge", "notice", "ui_click"] },
];

/** Page Réglages d'un invité (un compte les trouve dans son profil, voir `Profile`). */
export function Settings() {
  const t = useT();
  return (
    <main className="st-page">
      <p className="eyebrow">{t("settings.eyebrow")}</p>
      <h1 className="st-title">{t("settings.title")}</h1>
      <SettingsBody />
    </main>
  );
}

/** Apparence, sons et jeu : les sections de réglages, sans titre de page. */
export function SettingsBody() {
  const t = useT();
  const snd = useSoundSettings();
  const theme = useTheme();
  const off = !snd.enabled;

  return (
    <>
      <LanguageSettings />

      <section className="card st-card st-look" aria-labelledby="st-look">
        <div className="st-look-opts">
          <h2 id="st-look" className="st-h">{t("settings.look")}</h2>

          <fieldset className="st-field">
            <legend className="field-label">{t("settings.mode")}</legend>
            <div className="st-swatches" role="radiogroup" aria-label={t("settings.modeLabel")}>
              {COLOR_MODES.map((m) => (
                <button
                  key={m}
                  type="button"
                  role="radio"
                  aria-checked={theme.mode === m}
                  className="st-swatch"
                  data-sfx="off"
                  onClick={() => setTheme({ mode: m })}
                >
                  <span>{t(`settings.mode.${m}`)}</span>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="st-field">
            <legend className="field-label">{t("settings.board")}</legend>
            <div className="st-swatches" role="radiogroup" aria-label={t("settings.board")}>
              {BOARD_THEMES.map((b) => (
                <button
                  key={b.id}
                  type="button"
                  role="radio"
                  aria-checked={theme.board === b.id}
                  className="st-swatch"
                  data-sfx="off"
                  onClick={() => setTheme({ board: b.id })}
                >
                  <span className="st-mini" aria-hidden="true">
                    <i style={{ background: b.light }} />
                    <i style={{ background: b.dark }} />
                    <i style={{ background: b.dark }} />
                    <i style={{ background: b.light }} />
                  </span>
                  <span>{t(`settings.board.${b.id}`)}</span>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="st-field">
            <legend className="field-label">{t("settings.pieces")}</legend>
            <div className="st-swatches" role="radiogroup" aria-label={t("settings.pieces")}>
              {PIECE_SETS.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  role="radio"
                  aria-checked={theme.pieces === p.id}
                  className="st-swatch"
                  data-sfx="off"
                  onClick={() => setTheme({ pieces: p.id })}
                >
                  <span className="st-dots" aria-hidden="true">
                    <i style={{ background: p.white ?? "#ece8de" }} />
                    <i style={{ background: p.black ?? "#2a2c33", boxShadow: p.black ? undefined : "inset 0 0 0 1px #4a505b" }} />
                  </span>
                  <span>{t(`settings.pieces.${p.id}`)}</span>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="st-field">
            <legend className="field-label">{t("settings.accent")}</legend>
            <div className="st-swatches" role="radiogroup" aria-label={t("settings.accent")}>
              {ACCENTS.map((a) => (
                <button
                  key={a.id}
                  type="button"
                  role="radio"
                  aria-checked={theme.accent === a.id}
                  className="st-swatch"
                  data-sfx="off"
                  onClick={() => setTheme({ accent: a.id })}
                >
                  <span className="st-dots" aria-hidden="true">
                    <i style={{ background: a.color }} />
                  </span>
                  <span>{t(`settings.accent.${a.id}`)}</span>
                </button>
              ))}
            </div>
          </fieldset>
        </div>

        <div className="st-look-prev">
          <BoardPreview theme={theme} />
          <p className="st-legend muted">
            {t("settings.previewLegend")}
          </p>
        </div>
      </section>

      <section className="card st-card" aria-labelledby="st-sound">
        <h2 id="st-sound" className="st-h">{t("settings.sound")}</h2>
        <Switch checked={snd.enabled} onChange={(enabled) => sfx.setSettings({ enabled })} label={t("settings.soundOn")} hint={t("settings.soundOnHint")} />
        <Slider value={snd.master} onChange={(master) => sfx.setSettings({ master })} label={t("settings.volumeMaster")} disabled={off} />
        <Slider value={snd.effects} onChange={(effects) => sfx.setSettings({ effects })} label={t("settings.volumeEffects")} disabled={off} />
        <Switch checked={snd.ui} onChange={(ui) => sfx.setSettings({ ui })} label={t("settings.soundUi")} hint={t("settings.soundUiHint")} />
        <Switch checked={snd.yourTurn} onChange={(yourTurn) => sfx.setSettings({ yourTurn })} label={t("settings.yourTurn")} hint={t("settings.yourTurnHint")} />

        {off && <p className="st-note muted">{t("settings.soundOff")}</p>}
        <div className="st-groups">
          {GROUPS.map((g) => (
            <div key={g.id} className="st-group">
              <h3 className="eyebrow">{t(`settings.group.${g.id}`)}</h3>
              <div className="st-chips">
                {g.items.map((name) => (
                  <button key={name} type="button" className="st-play" disabled={off} data-sfx="off" onClick={() => preview(name)}>
                    <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true" focusable="false">
                      <path d="M3 1.8v8.4L10 6z" fill="currentColor" />
                    </svg>
                    {t(`settings.sfx.${name}`)}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>

        <h3 className="eyebrow st-skills-h">{t("settings.skills", { count: CATALOG.length })}</h3>
        <ul className="st-skills">
          {CATALOG.map((c) => (
            <li key={c.id}>
              <button
                type="button"
                className="st-skill"
                disabled={off}
                data-sfx="off"
                style={{ "--fam": `var(--fam-${c.family})` } as CSSProperties}
                onClick={() => preview(skillSfx(c.id))}
                aria-label={t("settings.listen", { name: c.name })}
              >
                <span className="st-skill-art">
                  <SkillArt id={c.id} size={34} />
                </span>
                <span className="st-skill-txt">
                  <span className="st-skill-name">{c.name}</span>
                  <span className="st-skill-fam">{FAMILY_LABEL[c.family]}</span>
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      <section className="card st-card" aria-labelledby="st-play">
        <h2 id="st-play" className="st-h">{t("settings.play")}</h2>
        <div className="st-row">
          <div className="st-row-txt">
            <span className="st-label" id="st-move-label">{t("settings.moveMode")}</span>
            <span className="st-hint muted">{t("settings.moveModeHint")}</span>
          </div>
          <div className="seg st-seg" role="radiogroup" aria-labelledby="st-move-label">
            {(["drag", "click"] as MoveMode[]).map((mode) => (
              <button
                key={mode}
                type="button"
                role="radio"
                aria-checked={theme.move === mode}
                aria-selected={theme.move === mode}
                data-sfx="off"
                onClick={() => setTheme({ move: mode })}
              >
                {t(`settings.move.${mode}`)}
              </button>
            ))}
          </div>
        </div>
        <Switch
          checked={theme.premove}
          onChange={(premove) => setTheme({ premove })}
          label={t("settings.premove")}
          hint={t("settings.premoveHint")}
        />
        <Switch
          checked={theme.reduceMotion}
          onChange={(reduceMotion) => setTheme({ reduceMotion })}
          label={t("settings.reduceMotion")}
          hint={t("settings.reduceMotionHint")}
        />
      </section>

      <ChatSettings />
    </>
  );
}

/** Chat : couper tous les messages reçus, et la liste des joueurs bloqués (comptes seulement). */
function ChatSettings() {
  const t = useT();
  const { account, blocked, connection } = useAppState();
  const online = connection === "open";
  const member = !!account && !account.guest;
  useEffect(() => {
    if (member && online) store.loadBlocks();
  }, [member, online, account?.player_id]);
  if (!account || account.guest) return null;
  return (
    <section className="card st-card" aria-labelledby="st-chat">
      <h2 id="st-chat" className="st-h">{t("settings.chat")}</h2>
      <Switch
        checked={!!account.chat_muted}
        onChange={(muted) => store.setChatMuted(muted)}
        label={t("settings.chatMute")}
        hint={t("settings.chatMuteHint")}
      />
      <h3 className="eyebrow st-skills-h">{t("settings.blocked", { count: blocked.length })}</h3>
      {blocked.length === 0 ? (
        <p className="st-hint muted">{t("settings.blockedNone")}</p>
      ) : (
        <ul className="mod-blocked">
          {blocked.map((name) => (
            <li key={name}>
              <span className="st-label">{name}</span>
              <button type="button" className="btn sm ghost" disabled={!online} onClick={() => store.unblockUser(name)}>
                {t("settings.unblock")}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** Langue de l'interface : « Automatique » suit le navigateur, un choix explicite est mémorisé. */
function LanguageSettings() {
  const t = useT();
  const lang = useLang();
  const auto = !hasChosenLang();
  return (
    <section className="card st-card" aria-labelledby="st-lang">
      <h2 id="st-lang" className="st-h">{t("settings.language")}</h2>
      <div className="st-row">
        <div className="st-row-txt">
          <span className="st-label" id="st-lang-label">{t("settings.languageLabel")}</span>
          <span className="st-hint muted">{auto ? t("settings.languageAutoHint", { name: LANG_NAMES[lang] }) : t("settings.languageHint")}</span>
        </div>
        <select
          className="input st-select"
          aria-labelledby="st-lang-label"
          value={auto ? "auto" : getLang()}
          onChange={(e) => setLang(e.target.value === "auto" ? null : (e.target.value as Lang))}
        >
          <option value="auto">{t("settings.languageAuto")}</option>
          {LANGS.map((l) => (
            <option key={l} value={l}>
              {LANG_NAMES[l]}
            </option>
          ))}
        </select>
      </div>
    </section>
  );
}
