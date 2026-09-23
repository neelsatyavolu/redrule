// Dev-only entry: mocks the backend, then boots the real app and opens the requested dialog.
import { useDialogs } from "../components/dialogs/dialogState";
import { useStore } from "../lib/store";
import { previewView } from "./preview";

await import("../main");

const whenLoaded = () =>
  new Promise<void>((resolve) => {
    const check = () => (useStore.getState().app ? resolve() : setTimeout(check, 20));
    check();
  });

await whenLoaded();
const meeting = useStore.getState().app?.meetings.find((m) => m.id === "M1");
if (previewView === "transcript") document.querySelectorAll<HTMLButtonElement>('[role="radio"]')[3]?.click();
if (previewView === "failed") useStore.getState().select("M2");
if (previewView === "settings") useDialogs.getState().open({ kind: "settings" });
if (previewView === "transcription") useDialogs.getState().open({ kind: "settings", tab: "transcription" });
if (previewView === "share" && meeting) useDialogs.getState().open({ kind: "share", meeting });
if (previewView === "edit" && meeting) useDialogs.getState().open({ kind: "edit", meeting });
