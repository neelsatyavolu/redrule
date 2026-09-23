import { create } from "zustand";
import type { FolderInfo, Meeting } from "../../lib/types";

export type OpenDialog =
  | { kind: "rename"; meeting: Meeting }
  | { kind: "tags"; meeting: Meeting }
  | { kind: "move"; meeting: Meeting }
  | { kind: "newFolder" }
  | { kind: "joinFolder" }
  | { kind: "renameFolder"; folder: FolderInfo }
  | { kind: "resetFolder" | "deleteFolder" | "leaveFolder"; folder: FolderInfo }
  | { kind: "edit"; meeting: Meeting }
  | { kind: "share"; meeting: Meeting }
  | { kind: "delete"; meeting: Meeting }
  | { kind: "settings"; tab?: SettingsTab };

export type SettingsTab = "general" | "transcription" | "accounts" | "permissions";

interface DialogState {
  current: OpenDialog | null;
  open: (dialog: OpenDialog) => void;
  close: () => void;
}

/** One dialog at a time, opened from menus, the toolbar or the sidebar. */
export const useDialogs = create<DialogState>((set) => ({
  current: null,
  open: (current) => set({ current }),
  close: () => set({ current: null }),
}));
