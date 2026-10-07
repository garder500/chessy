import { describe, expect, it } from "vitest";
import { parsePieceBin } from "./HeroPiece3D";

describe("parsePieceBin", () => {
  it("lit positions, normales et indices", () => {
    const pos = [0, 0, 0, 1, 0, 0, 0, 1, 0];
    const nor = [0, 0, 1, 0, 0, 1, 0, 0, 1];
    const buf = new ArrayBuffer(8 + 9 * 4 * 2 + 3 * 4);
    new Uint32Array(buf, 0, 2).set([3, 3]);
    new Float32Array(buf, 8, 9).set(pos);
    new Float32Array(buf, 8 + 36, 9).set(nor);
    new Uint32Array(buf, 8 + 72, 3).set([0, 1, 2]);
    const geo = parsePieceBin(buf);
    expect(Array.from(geo.getAttribute("position").array)).toEqual(pos);
    expect(Array.from(geo.getAttribute("normal").array)).toEqual(nor);
    expect(Array.from(geo.getIndex()!.array)).toEqual([0, 1, 2]);
  });
});
