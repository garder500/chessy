import { STAR_LABELS } from "../campaign";
import "./stars.css";

interface Props {
  /** Étoiles gagnées (affichées pleines). */
  stars: readonly boolean[];
  /** Étoiles déjà acquises avant (affichées atténuées quand `stars` ne les a pas). */
  kept?: readonly boolean[];
  size?: number;
  /** Le boss n'offre que l'étoile de victoire. */
  boss?: boolean;
}

const STAR_PATH = "M12 2.8l2.8 5.9 6.4.9-4.6 4.5 1.1 6.4L12 17.5l-5.7 3 1.1-6.4L2.8 9.6l6.4-.9z";

/** Les étoiles d'un niveau de campagne : victoire, objectif, défi (victoire seule pour le boss). */
export function Stars({ stars, kept = [], size = 18, boss = false }: Props) {
  const labels = boss ? STAR_LABELS.slice(0, 1) : STAR_LABELS;
  const won = stars.filter(Boolean).length;
  return (
    <span className="stars" role="img" aria-label={`${won} étoile${won > 1 ? "s" : ""} sur ${labels.length}`}>
      {labels.map((label, i) => (
        <svg key={label} className={`star${stars[i] ? " on" : kept[i] ? " kept" : ""}`} viewBox="0 0 24 24" width={size} height={size} aria-hidden="true">
          <path d={STAR_PATH} />
        </svg>
      ))}
    </span>
  );
}
