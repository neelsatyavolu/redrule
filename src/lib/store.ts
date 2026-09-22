import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { create } from "zustand";
import { api, errorMessage } from "./api";
import type { AppState, Meeting } from "./types";

export type Library = "meetings" | "archive";

interface UiState {
  app: AppState | null;
  selectedId: string | null;
  library: Library;
  search: string;
  select: (id: string | null) => void;
  setLibrary: (library: Library) => void;
  setSearch: (search: string) => void;
}

export const useStore = create<UiState>((set) => ({
  app: null,
  selectedId: null,
  library: "meetings",
  search: "",
  select: (selectedId) => set({ selectedId }),
  setLibrary: (library) => set({ library }),
  setSearch: (search) => set({ search }),
}));

export function visibleMeetings(app: AppState | null, library: Library): Meeting[] {
  if (!app) return [];
  return app.meetings.filter((m) => Boolean(m.archivedAt) === (library === "archive"));
}

/** Keeps the selection pointing at something that exists, and follows a new recording. */
function reconcile(next: AppState, previous: AppState | null) {
  const { selectedId, library, select, setLibrary } = useStore.getState();
  if (next.recordingId && next.recordingId !== previous?.recordingId) {
    setLibrary("meetings");
    select(next.recordingId);
    return;
  }
  // A meeting that was deleted, or archived out of the current list, hands the selection on.
  const visible = visibleMeetings(next, library);
  if (!selectedId || !visible.some((m) => m.id === selectedId)) {
    select(visible[0]?.id ?? null);
  }
}

/** Mirrors the backend's state into the store for the life of the window. */
export async function connectToBackend(): Promise<() => void> {
  const apply = (next: AppState) => {
    const previous = useStore.getState().app;
    useStore.setState({ app: next });
    reconcile(next, previous);
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
  return meeting.id !== app?.recordingId && (meeting.status === "done" || meeting.status === "failed");
}
