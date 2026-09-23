import { describe, expect, it } from "vitest";
import { folderScope, keptTag, scopeFolder, visibleMeetings, withFolderMeetings } from "./library";
import type { AppState, Meeting } from "./types";

const FOLDER = "f".repeat(64);

function meeting(id: string, overrides: Partial<Meeting> = {}): Meeting {
  return { id, title: id, app: "zoom", startedAt: "2026-09-22T09:00:00Z", status: "done", ...overrides };
}

const app = withFolderMeetings({
  meetings: [
    meeting("mine"),
    meeting("old", { archivedAt: "2026-09-22T10:00:00Z", tags: ["Acme"] }),
    meeting("shared", { folderId: FOLDER, tags: ["Acme"] }),
  ],
  folderMeetings: [meeting("theirs", { folderId: FOLDER, remote: true, recordedBy: "Dana" })],
} as unknown as AppState);

const ids = (meetings: Meeting[]) => meetings.map((m) => m.id);

describe("library scopes", () => {
  it("round-trips a folder scope", () => {
    expect(scopeFolder(folderScope(FOLDER))).toBe(FOLDER);
    expect(scopeFolder("meetings")).toBeNull();
  });

  it("lists this Mac's meetings in My meetings and Archive, and everyone's in a folder", () => {
    expect(ids(visibleMeetings(app, "meetings"))).toEqual(["mine", "shared"]);
    expect(ids(visibleMeetings(app, "archive"))).toEqual(["old"]);
    expect(ids(visibleMeetings(app, folderScope(FOLDER)))).toEqual(["shared", "theirs"]);
  });

  it("filters by tag within the scope and drops a tag no meeting there has", () => {
    expect(ids(visibleMeetings(app, folderScope(FOLDER), "acme"))).toEqual(["shared"]);
    expect(keptTag(app, "archive", "Acme")).toBe("Acme");
    expect(keptTag(app, "meetings", "Hiring")).toBeNull();
  });
});
