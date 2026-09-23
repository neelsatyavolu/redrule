import { describe, expect, it } from "vitest";
import {
  allTags,
  clock,
  distinctSpeakers,
  groupByDay,
  hasTag,
  length,
  matchesSearch,
  meetingSentence,
  speakerKey,
  speakerLabel,
} from "./format";
import type { Meeting, TranscriptSegment } from "./types";

function meeting(overrides: Partial<Meeting>): Meeting {
  return { id: "A", title: "Standup", app: "zoom", startedAt: "2026-09-22T09:00:00Z", status: "done", ...overrides };
}

describe("clock", () => {
  it("pads minutes and seconds and adds hours when needed", () => {
    expect(clock(65.9)).toBe("01:05");
    expect(clock(3725)).toBe("1:02:05");
    expect(clock(-3)).toBe("00:00");
  });
});

describe("length", () => {
  it("reads like a sentence", () => {
    expect(length(20)).toBe("under a minute");
    expect(length(60)).toBe("1 minute");
    expect(length(42 * 60)).toBe("42 minutes");
    expect(length(120 * 60)).toBe("2 hr");
    expect(length(65 * 60)).toBe("1 hr 5 min");
  });
});

describe("meetingSentence", () => {
  it("includes the length and the app, except for manual recordings", () => {
    const zoom = meeting({ endedAt: "2026-09-22T09:42:00Z" });
    expect(meetingSentence(zoom)).toMatch(/, 42 minutes on Zoom$/);
    expect(meetingSentence(meeting({ app: "manual" }))).not.toContain(" on ");
  });
});

describe("groupByDay", () => {
  it("labels today and yesterday and orders newest first", () => {
    const now = new Date(2026, 8, 22, 18);
    const groups = groupByDay(
      [
        meeting({ id: "1", startedAt: new Date(2026, 8, 21, 10).toISOString() }),
        meeting({ id: "2", startedAt: new Date(2026, 8, 22, 9).toISOString() }),
        meeting({ id: "3", startedAt: new Date(2026, 8, 22, 15).toISOString() }),
        meeting({ id: "4", startedAt: new Date(2026, 8, 10, 15).toISOString() }),
      ],
      now,
    );
    expect(groups.map((g) => g.label).slice(0, 2)).toEqual(["Today", "Yesterday"]);
    expect(groups[0].meetings.map((m) => m.id)).toEqual(["3", "2"]);
    expect(groups).toHaveLength(3);
  });
});

describe("speakers", () => {
  const them1: TranscriptSegment = { speaker: "them", start: 0, end: 1, text: "a", speakerID: "1" };
  const me: TranscriptSegment = { speaker: "me", start: 2, end: 3, text: "b" };

  it("labels by name, then number, then side", () => {
    expect(speakerLabel(me)).toBe("Me");
    expect(speakerLabel(them1)).toBe("Speaker 1");
    expect(speakerLabel({ ...them1, speakerName: "Ada" })).toBe("Ada");
    expect(speakerKey(them1)).toBe("them:1");
    expect(speakerKey(me)).toBe("me:source");
  });

  it("lists each speaker once in order of appearance", () => {
    expect(distinctSpeakers([them1, me, { ...them1, start: 5 }]).map(speakerKey)).toEqual(["them:1", "me:source"]);
  });
});

describe("matchesSearch", () => {
  it("ignores case and blank queries", () => {
    expect(matchesSearch(meeting({ title: "Roadmap review" }), "ROAD")).toBe(true);
    expect(matchesSearch(meeting({}), "  ")).toBe(true);
    expect(matchesSearch(meeting({}), "budget")).toBe(false);
  });
});

describe("tags", () => {
  it("lists each tag once, ignoring case, in alphabetical order", () => {
    const meetings = [meeting({ tags: ["hiring", "Acme"] }), meeting({ tags: ["acme"] }), meeting({})];
    expect(allTags(meetings)).toEqual(["Acme", "hiring"]);
  });

  it("matches a tag regardless of case", () => {
    expect(hasTag(meeting({ tags: ["Acme"] }), "ACME")).toBe(true);
    expect(hasTag(meeting({}), "Acme")).toBe(false);
  });
});
