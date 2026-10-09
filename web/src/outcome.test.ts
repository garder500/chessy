import { beforeAll, describe, expect, it } from "vitest";
import { setLang } from "./i18n";
import { describeOutcome, formatDelta, resultFor, resultHeadline } from "./outcome";

beforeAll(() => setLang("fr"));

describe("outcome", () => {
  it("describes the new endings from either side", () => {
    expect(describeOutcome({ type: "timeout", winner: "white" }, "white")).toMatch(/victoire/i);
    expect(describeOutcome({ type: "timeout", winner: "white" }, "black")).toMatch(/temps/i);
    expect(describeOutcome({ type: "draw_agreed" }, "white")).toMatch(/nulle/i);
  });

  it("derives the result and headline for the player", () => {
    expect(resultFor({ type: "timeout", winner: "black" }, "white")).toBe("loss");
    expect(resultFor({ type: "draw_agreed" }, "white")).toBe("draw");
    expect(resultFor({ type: "ongoing" }, "white")).toBeNull();
    expect(resultHeadline({ type: "checkmate", winner: "white" }, "white")).toEqual({ title: "Victoire", reason: "Échec et mat" });
    expect(resultHeadline({ type: "resignation", winner: "black" }, "white")).toEqual({ title: "Défaite", reason: "Vous avez abandonné" });
    expect(resultHeadline({ type: "stalemate" }, "black").title).toBe("Partie nulle");
  });

  it("signs Elo changes with a real minus", () => {
    expect(formatDelta(14)).toBe("+14");
    expect(formatDelta(-9)).toBe("−9");
    expect(formatDelta(0)).toBe("±0");
  });
});
