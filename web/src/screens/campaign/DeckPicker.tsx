import { MAX_DECK_PICKS, splitDeck } from "../../campaign";
import type { SkillId } from "../../protocol";
import { SkillCard } from "../SkillCard";

interface Props {
  deck: SkillId[];
  /** Compétences du chapitre prêtées pour la partie : hors deck, hors collection. */
  lent: SkillId[];
  picked: SkillId[];
  onToggle: (skill: SkillId) => void;
}

/** Choix de trois classiques parmi le deck du joueur et les compétences prêtées ; les uniques du deck viennent en plus. */
export function DeckPicker({ deck, lent, picked, onToggle }: Props) {
  const { pickable, extras } = splitDeck(deck, lent);
  if (pickable.length === 0) {
    return <p className="muted">Votre deck est vide : gagnez des compétences en partie classée avant de tenter ce niveau.</p>;
  }
  return (
    <div className="cp-deck">
      <p className="lab">
        Choisissez {MAX_DECK_PICKS} classiques · {picked.length}/{MAX_DECK_PICKS}
      </p>
      <div className="cp-pick-grid">
        {pickable.map((skill) => {
          const rank = picked.indexOf(skill);
          return (
            <SkillCard
              key={skill}
              skill={skill}
              selected={rank >= 0}
              order={rank >= 0 ? rank + 1 : undefined}
              badge={lent.includes(skill) ? "prêtée" : undefined}
              disabled={rank < 0 && picked.length >= MAX_DECK_PICKS}
              onClick={() => onToggle(skill)}
            />
          );
        })}
      </div>
      {extras.length > 0 && (
        <>
          <p className="lab">Uniques de votre deck, en plus</p>
          <div className="cp-pick-grid">
            {extras.map((skill) => (
              <SkillCard key={skill} skill={skill} badge="en plus" />
            ))}
          </div>
        </>
      )}
    </div>
  );
}
