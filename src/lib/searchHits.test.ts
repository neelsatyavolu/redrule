import { describe, expect, it } from "vitest";
import { applicableHits } from "./searchHits";

describe("applicableHits", () => {
  const budget = new Map<string, string | null>([["A", "…the budget moved…"]]);

  it("ignores hits that belong to a different query", () => {
    expect(applicableHits("zzzz", { query: "budget", hits: budget })?.size).toBe(0);
    expect(applicableHits("budget", { query: "budget", hits: budget })).toBe(budget);
  });

  it("returns nothing to apply while the box is empty", () => {
    expect(applicableHits("  ", { query: "budget", hits: budget })).toBeNull();
    expect(applicableHits("budget", null)?.size).toBe(0);
  });
});
