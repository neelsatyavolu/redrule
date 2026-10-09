import { hasTag } from "./format";
import type { AppState, Meeting } from "./types";

/** What the sidebar lists: this Mac's meetings, its archive, or one shared folder. */
export type Library = "meetings" | "archive" | `folder:${string}`;

export function folderScope(id: string): Library {
  return `folder:${id}`;
}

export function scopeFolder(library: Library): string | null {
  return library.startsWith("folder:") ? library.slice("folder:".length) : null;
}

/** Adds other people's folder meetings to the list, so the whole UI sees one set of meetings. */
export function withFolderMeetings(state: AppState): AppState {
  return { ...state, meetings: [...state.meetings, ...(state.folderMeetings ?? [])] };
}

export function visibleMeetings(app: AppState | null, library: Library, tag: string | null = null): Meeting[] {
  if (!app) return [];
  const folder = scopeFolder(library);
  return app.meetings.filter(
    (m) =>
      (folder !== null ? m.folderId === folder : !m.remote && Boolean(m.archivedAt) === (library === "archive")) &&
      (tag === null || hasTag(m, tag)),
  );
}

/** The tag filter, or null once no meeting in the library carries it any more. */
export function keptTag(app: AppState | null, library: Library, tag: string | null): string | null {
  return tag !== null && visibleMeetings(app, library, tag).length > 0 ? tag : null;
}

/** What the detail pane says when the current list has nothing selected. */
export function emptyLibraryMessage(library: Library, searching: boolean): { title: string; detail: string; record: boolean } {
  if (searching) {
    return { title: "No meetings match", detail: "Try a different word, or clear the search.", record: false };
  }
  if (scopeFolder(library) !== null) {
    return {
      title: "No meetings in this folder",
      detail: "Record with the folder open, or move a meeting here from its menu.",
      record: true,
    };
  }
  if (library === "archive") {
    return {
      title: "Nothing archived",
      detail: "Archive a meeting from its menu when you're done with it.",
      record: false,
    };
  }
  return {
    title: "No meetings yet",
    detail:
      "Join a Zoom or Google Meet call and Redrule offers to take notes. It listens to the call and your microphone, transcribes on this Mac, and writes up the meeting when it ends.",
    record: true,
  };
}
