import { describe, expect, it } from "vitest";
import { lostSkillLines } from "./LostSkill";

describe("lostSkillLines", () => {
  it("décrit une compétence prise", () => {
    expect(lostSkillLines({ by: "Ana", kind: "stolen", skill: "freeze", refilled: null })).toEqual(["Ana vous a pris Freeze."]);
  });

  it("décrit une compétence perdue au hasard par la forge", () => {
    expect(lostSkillLines({ by: "Ana", kind: "forged", skill: "tornado", refilled: null })).toEqual(["Ana a forgé : vous perdez Tornado (au hasard)."]);
  });

  it("décrit une épargne et un deck rempli", () => {
    expect(lostSkillLines({ by: "Ana", kind: "spared", skill: null, refilled: "freeze" })).toEqual([
      "Ana vous a épargné.",
      "Votre deck était vide : vous recevez Freeze.",
    ]);
  });
});
