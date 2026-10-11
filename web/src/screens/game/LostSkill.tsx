import type { RewardOutcomeKind, SkillId } from "../../protocol";
import { skillName } from "../../skills";
import { useAppState } from "../../store";

export interface RewardOutcomeInfo {
  by: string;
  kind: RewardOutcomeKind;
  skill: SkillId | null;
  refilled: SkillId | null;
}

/** Phrases décrivant l'issue de la récompense classée du point de vue du perdant. */
export function lostSkillLines({ by, kind, skill, refilled }: RewardOutcomeInfo): string[] {
  const lost = skill ? skillName(skill) : "une compétence";
  const main =
    kind === "stolen" ? `${by} vous a pris ${lost}.` : kind === "forged" ? `${by} a forgé : vous perdez ${lost} (au hasard).` : `${by} vous a épargné.`;
  return refilled ? [main, `Votre deck était vide : vous recevez ${skillName(refilled)}.`] : [main];
}

/** Issue de la récompense du gagnant, affichée au perdant sur l'écran de fin. */
export function LostSkill() {
  const { rewardOutcome } = useAppState();
  if (!rewardOutcome) return null;
  return (
    <div className="card rs-lostskill" role="status">
      {lostSkillLines(rewardOutcome).map((line) => (
        <p key={line}>{line}</p>
      ))}
    </div>
  );
}
