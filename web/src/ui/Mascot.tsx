import { useT } from "../i18n";

/** Le Chevalier : mascotte de Chessy. Cavalier « chess-knight » de Skoll (game-icons.net, CC BY 3.0), couronne « Crown5 » de Reicon (MIT). */
export function Mascot({ size = 164 }: { size?: number }) {
  const t = useT();
  return (
    <div className="mascot" style={{ width: size, height: size }} role="img" aria-label={t("game.mascot_alt")}>
      <svg className="mascot-knight" viewBox="0 0 512 512" aria-hidden="true" focusable="false">
        <defs>
          <linearGradient id="mk-gold" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#fff3c9" />
            <stop offset=".45" stopColor="#e8b64f" />
            <stop offset="1" stopColor="#8a5e12" />
          </linearGradient>
        </defs>
        <path
          fill="url(#mk-gold)"
          stroke="#4a2f06"
          strokeWidth="7"
          strokeLinejoin="round"
          d="M60.81 476.91h300v-60h-300v60zm233.79-347.3l13.94 7.39c31.88-43.62 61.34-31.85 61.34-31.85l-21.62 53 35.64 19 2.87 33 64.42 108.75-43.55 29.37s-26.82-36.39-39.65-43.66c-10.66-6-41.22-10.25-56.17-12l-67.54-76.91-12 10.56 37.15 42.31c-.13.18-.25.37-.38.57-35.78 58.17 23 105.69 68.49 131.78H84.14C93 85 294.6 129.61 294.6 129.61z"
        />
      </svg>
      <svg className="mascot-crown" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
        <path d="M17 22H7a.75.75 0 010-1.5h10a.75.75 0 010 1.5z" fill="url(#mk-gold)" />
        <path d="M20.35 5.52l-4 2.86c-.53.38-1.29.15-1.52-.46l-1.89-5.04c-.32-.87-1.55-.87-1.87 0l-1.9 5.03c-.23.62-.98.85-1.51.46l-4-2.86c-.8-.56-1.86.23-1.53 1.16l4.16 11.65c.14.4.52.66.94.66h9.53c.42 0 .8-.27.94-.66l4.16-11.65c.34-.93-.72-1.72-1.51-1.16zM14.5 14.75h-5a.75.75 0 010-1.5h5a.75.75 0 010 1.5z" fill="url(#mk-gold)" />
      </svg>
    </div>
  );
}
