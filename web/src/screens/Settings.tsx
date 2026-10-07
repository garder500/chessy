import { useSyncExternalStore, type CSSProperties } from "react";
import { CATALOG, FAMILY_LABEL } from "../catalog";
import { sfx, skillSfx, type SfxName } from "../sound";
import { ACCENTS, BOARD_THEMES, PIECE_SETS, setTheme, useTheme, type ColorMode, type MoveMode } from "../theme";
import { BoardPreview } from "../ui/BoardPreview";
import { SkillArt } from "../ui/SkillArt";
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

const COLOR_MODES: { id: ColorMode; label: string }[] = [
  { id: "system", label: "Système" },
  { id: "light", label: "Clair" },
  { id: "dark", label: "Sombre" },
];

/** Aperçu d'un son : ignore les interrupteurs de catégorie, jamais le réglage général. */
const preview = (name: SfxName) => sfx.play(name, { force: true });

const GROUPS: { title: string; items: [SfxName, string][] }[] = [
  {
    title: "Coups",
    items: [
      ["move", "Coup"],
      ["capture", "Capture"],
      ["castle", "Roque"],
      ["promote", "Promotion"],
      ["check", "Échec"],
      ["illegal", "Coup refusé"],
    ],
  },
  {
    title: "Partie",
    items: [
      ["game_start", "Début"],
      ["your_turn", "À vous de jouer"],
      ["low_time", "Peu de temps"],
      ["game_win", "Victoire"],
      ["game_lose", "Défaite"],
      ["game_draw", "Nulle"],
    ],
  },
  {
    title: "Effets",
    items: [
      ["trap_sprung", "Piège déclenché"],
      ["shield", "Bouclier"],
      ["pushed", "Repoussé"],
      ["saved", "Sauvé"],
      ["vanish", "Disparition"],
    ],
  },
  {
    title: "Social et interface",
    items: [
      ["match_found", "Adversaire trouvé"],
      ["chat", "Message"],
      ["friend_request", "Demande d'ami"],
      ["challenge", "Défi reçu"],
      ["notice", "Notification"],
      ["ui_click", "Clic"],
    ],
  },
];

export function Settings() {
  const snd = useSoundSettings();
  const theme = useTheme();
  const off = !snd.enabled;

  return (
    <main className="st-page">
      <p className="eyebrow">Préférences</p>
      <h1 className="st-title">Réglages</h1>

      <section className="card st-card st-look" aria-labelledby="st-look">
        <div className="st-look-opts">
          <h2 id="st-look" className="st-h">Apparence</h2>

          <fieldset className="st-field">
            <legend className="field-label">Mode</legend>
            <div className="st-swatches" role="radiogroup" aria-label="Mode clair ou sombre">
              {COLOR_MODES.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  role="radio"
                  aria-checked={theme.mode === m.id}
                  className="st-swatch"
                  data-sfx="off"
                  onClick={() => setTheme({ mode: m.id })}
                >
                  <span>{m.label}</span>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="st-field">
            <legend className="field-label">Thème du plateau</legend>
            <div className="st-swatches" role="radiogroup" aria-label="Thème du plateau">
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
                  <span>{b.label}</span>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="st-field">
            <legend className="field-label">Jeu de pièces</legend>
            <div className="st-swatches" role="radiogroup" aria-label="Jeu de pièces">
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
                  <span>{p.label}</span>
                </button>
              ))}
            </div>
          </fieldset>

          <fieldset className="st-field">
            <legend className="field-label">Couleur d'accent</legend>
            <div className="st-swatches" role="radiogroup" aria-label="Couleur d'accent">
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
                  <span>{a.label}</span>
                </button>
              ))}
            </div>
          </fieldset>
        </div>

        <div className="st-look-prev">
          <BoardPreview theme={theme} />
          <p className="st-legend muted">
            Aperçu : dernier coup (accent), cases légales et capture, premove (accent mêlé de rouge).
          </p>
        </div>
      </section>

      <section className="card st-card" aria-labelledby="st-sound">
        <h2 id="st-sound" className="st-h">Sons</h2>
        <Switch checked={snd.enabled} onChange={(enabled) => sfx.setSettings({ enabled })} label="Sons activés" hint="Interrupteur général : coupe tous les effets." />
        <Slider value={snd.master} onChange={(master) => sfx.setSettings({ master })} label="Volume général" disabled={off} />
        <Slider value={snd.effects} onChange={(effects) => sfx.setSettings({ effects })} label="Volume des effets" disabled={off} />
        <Switch checked={snd.ui} onChange={(ui) => sfx.setSettings({ ui })} label="Sons d'interface" hint="Clics, messages, demandes d'ami, défis, notifications." />
        <Switch checked={snd.yourTurn} onChange={(yourTurn) => sfx.setSettings({ yourTurn })} label="Notification « à vous de jouer »" hint="Un petit carillon quand le trait vous revient." />

        {off && <p className="st-note muted">Les sons sont désactivés : activez-les pour écouter les aperçus.</p>}
        <div className="st-groups">
          {GROUPS.map((g) => (
            <div key={g.title} className="st-group">
              <h3 className="eyebrow">{g.title}</h3>
              <div className="st-chips">
                {g.items.map(([name, label]) => (
                  <button key={name} type="button" className="st-play" disabled={off} data-sfx="off" onClick={() => preview(name)}>
                    <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true" focusable="false">
                      <path d="M3 1.8v8.4L10 6z" fill="currentColor" />
                    </svg>
                    {label}
                  </button>
                ))}
              </div>
            </div>
          ))}
        </div>

        <h3 className="eyebrow st-skills-h">Compétences ({CATALOG.length})</h3>
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
                aria-label={`Écouter ${c.name}`}
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
        <h2 id="st-play" className="st-h">Jeu</h2>
        <div className="st-row">
          <div className="st-row-txt">
            <span className="st-label" id="st-move-label">Mode de déplacement</span>
            <span className="st-hint muted">Le clic sur une pièce puis sur sa destination fonctionne toujours.</span>
          </div>
          <div className="seg st-seg" role="radiogroup" aria-labelledby="st-move-label">
            {([["drag", "Glisser-déposer + clic"], ["click", "Clic seulement"]] as [MoveMode, string][]).map(([mode, label]) => (
              <button
                key={mode}
                type="button"
                role="radio"
                aria-checked={theme.move === mode}
                aria-selected={theme.move === mode}
                data-sfx="off"
                onClick={() => setTheme({ move: mode })}
              >
                {label}
              </button>
            ))}
          </div>
        </div>
        <Switch
          checked={theme.premove}
          onChange={(premove) => setTheme({ premove })}
          label="Premoves"
          hint="Préparer un coup pendant le tour de l'adversaire ; il part dès que c'est à vous s'il est légal."
        />
        <Switch
          checked={theme.reduceMotion}
          onChange={(reduceMotion) => setTheme({ reduceMotion })}
          label="Réduire les animations"
          hint="Plateau et interface presque instantanés, sans secousses."
        />
      </section>
    </main>
  );
}
