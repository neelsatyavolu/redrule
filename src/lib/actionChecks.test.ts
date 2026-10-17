import { describe, expect, it } from "vitest";
import { beginCheck, finishCheck } from "./actionChecks";

const open = [{ task: "A", done: false }, { task: "B", done: false }];
const first = [{ task: "A", done: true }, { task: "B", done: false }];
const both = [{ task: "A", done: true }, { task: "B", done: true }];

describe("action item checks", () => {
  it("keeps a later check when an earlier save finishes last", () => {
    const started = beginCheck({ shown: open, saved: open }, first);
    const movedOn = beginCheck(started, both);
    const finished = finishCheck(movedOn, first, true);
    expect(finished.shown).toBe(both);
    expect(finished.saved).toBe(first);
    expect(finishCheck(finished, both, true)).toEqual({ shown: both, saved: both });
  });

  it("rolls back only the attempt that is still on screen", () => {
    const started = beginCheck({ shown: open, saved: open }, first);
    expect(finishCheck(started, first, false)).toEqual({ shown: open, saved: open });
    const movedOn = beginCheck(started, both);
    expect(finishCheck(movedOn, first, false)).toEqual(movedOn);
  });
});
