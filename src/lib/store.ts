import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { create } from "zustand";
import { api, errorMessage } from "./api";
import { folderScope, keptTag, scopeFolder, visibleMeetings, withFolderMeetings, type Library } from "./library";
import type { AppState, Meeting } from "./types";

export { folderScope, keptTag, scopeFolder, visibleMeetings, type Library };

interface UiState {
  app: AppState | null;
  selectedId: string | null;
  library: Library;
  search: string;
  /** Shows only meetings with this tag; null shows all. */
  tag: string | null;
  select: (id: string | null) => void;
  setLibrary: (library: Library) => void;
  setSearch: (search: string) => void;
  setTag: (tag: string | null) => void;
}

export const useStore = create<UiState>((set) => ({
  app: null,
  selectedId: null,
  library: "meetings",
  search: "",
  tag: null,
  select: (selectedId) => set({ selectedId }),
  setLibrary: (library) => set({ library }),
  setSearch: (search) => set({ search }),
  setTag: (tag) => set({ tag }),
}));

/** Keeps the selection pointing at something that exists, and follows a new recording. */
function reconcile(next: AppState, previous: AppState | null) {
  const { selectedId, select, setLibrary, setTag } = useStore.getState();
  if (next.recordingId && next.recordingId !== previous?.recordingId) {
    const folder = next.meetings.find((m) => m.id === next.recordingId)?.folderId;
    setLibrary(folder ? folderScope(folder) : "meetings");
    setTag(null);
    select(next.recordingId);
    return;
  }
  // A folder that was left or deleted hands back to My meetings.
  const folder = scopeFolder(useStore.getState().library);
  if (folder !== null && !next.folders.some((f) => f.id === folder)) setLibrary("meetings");
  const library = useStore.getState().library;
  // Untagging the last meeting with the filtered tag clears the filter.
  const tag = keptTag(next, library, useStore.getState().tag);
  setTag(tag);
  // A meeting that was deleted, or archived out of the current list, hands the selection on.
  const visible = visibleMeetings(next, library, tag);
  if (!selectedId || !visible.some((m) => m.id === selectedId)) {
    select(visible[0]?.id ?? null);
  }
}

/** Mirrors the backend's state into the store for the life of the window. */
export async function connectToBackend(): Promise<() => void> {
  const apply = (next: AppState) => {
    const previous = useStore.getState().app;
    const merged = withFolderMeetings(next);
    useStore.setState({ app: merged });
    reconcile(merged, previous);
  };
  const unlistenState = await listen<AppState>("state", (event) => apply(event.payload));
  const unlistenError = await listen<string>("app-error", (event) => toast.error(event.payload));
  apply(await api.getState());
  return () => {
    unlistenState();
    unlistenError();
  };
}

/** Runs a command and shows its error, if any, as a toast. Returns whether it succeeded. */
export async function attempt(work: () => Promise<unknown>, success?: string): Promise<boolean> {
  try {
    await work();
    if (success) toast.success(success);
    return true;
  } catch (error) {
    toast.error(errorMessage(error));
    return false;
  }
}

export function canEdit(app: AppState | null, meeting: Meeting): boolean {
  return !meeting.remote && meeting.id !== app?.recordingId && (meeting.status === "done" || meeting.status === "failed");
}
