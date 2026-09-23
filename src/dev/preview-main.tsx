// Dev-only entry: mocks the backend, then boots the real app and opens the requested dialog.
import { useDialogs } from "../components/dialogs/dialogState";
import { folderScope, useStore } from "../lib/store";
import { previewView } from "./preview";

await import("../main");

const whenLoaded = () =>
  new Promise<void>((resolve) => {
    const check = () => (useStore.getState().app ? resolve() : setTimeout(check, 20));
    check();
  });

await whenLoaded();
const meeting = useStore.getState().app?.meetings.find((m) => m.id === "M1");
if (previewView === "transcript") document.querySelectorAll<HTMLButtonElement>('[aria-label="Meeting content"] [role="radio"]')[1]?.click();
if (previewView === "failed") useStore.getState().select("M2");
if (previewView === "settings") useDialogs.getState().open({ kind: "settings" });
if (previewView === "transcription") useDialogs.getState().open({ kind: "settings", tab: "transcription" });
if (previewView === "accounts") useDialogs.getState().open({ kind: "settings", tab: "accounts" });
if (previewView === "share" && meeting) useDialogs.getState().open({ kind: "share", meeting });
if (previewView === "folder") {
  useStore.getState().setLibrary(folderScope("f".repeat(64)));
  useStore.getState().select("R1");
}
if (previewView === "search") useStore.getState().setSearch("free tier");
if (previewView === "newFolder") useDialogs.getState().open({ kind: "newFolder" });
if (previewView === "move" && meeting) useDialogs.getState().open({ kind: "move", meeting });
if (previewView === "tags" && meeting) useDialogs.getState().open({ kind: "tags", meeting });
if (previewView === "edit" && meeting) useDialogs.getState().open({ kind: "edit", meeting });
