import { create } from "zustand";
import type { Meeting } from "../../lib/types";

export type OpenDialog =
  | { kind: "rename"; meeting: Meeting }
  | { kind: "edit"; meeting: Meeting }
  | { kind: "share"; meeting: Meeting }
  | { kind: "delete"; meeting: Meeting }
  | { kind: "settings"; tab?: SettingsTab };

export type SettingsTab = "general" | "accounts" | "permissions";

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
