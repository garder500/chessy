import { MAX_DECK_PICKS } from "../../campaign";
import type { SkillId } from "../../protocol";
import { SkillCard } from "../SkillCard";

interface Props {
  deck: SkillId[];
  picked: SkillId[];
  onToggle: (skill: SkillId) => void;
}

/** Choix des compétences emmenées dans un niveau, parmi le deck actuel du joueur. */
export function DeckPicker({ deck, picked, onToggle }: Props) {
  if (deck.length === 0) {
    return <p className="muted">Votre deck est vide : gagnez des compétences en partie classée avant de tenter ce niveau.</p>;
  }
  return (
    <div className="cp-deck">
      <p className="lab">
        Choisissez jusqu'à {MAX_DECK_PICKS} compétences · {picked.length}/{MAX_DECK_PICKS}
      </p>
      <div className="cp-pick-grid">
        {deck.map((skill) => {
          const rank = picked.indexOf(skill);
          return (
            <SkillCard
              key={skill}
              skill={skill}
              selected={rank >= 0}
              order={rank >= 0 ? rank + 1 : undefined}
              disabled={rank < 0 && picked.length >= MAX_DECK_PICKS}
              onClick={() => onToggle(skill)}
            />
          );
        })}
      </div>
    </div>
  );
}
