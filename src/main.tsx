import { listen } from "@tauri-apps/api/event";
import { StrictMode, useEffect } from "react";
import { createRoot } from "react-dom/client";
import { Toaster, toast } from "sonner";
import { DialogHost } from "./components/dialogs/DialogHost";
import { useDialogs } from "./components/dialogs/dialogState";
import { MeetingPane } from "./components/MeetingPane";
import { Sidebar } from "./components/Sidebar";
import { api } from "./lib/api";
import { connectToBackend, useStore } from "./lib/store";
import "./styles.css";

function App() {
  const storageError = useStore((s) => s.app?.storageError);

  useEffect(() => {
    let disconnect: (() => void) | undefined;
    let cancelled = false;
    connectToBackend().then((stop) => {
      if (cancelled) stop();
      else disconnect = stop;
      // Show the window only once there is something to paint.
      requestAnimationFrame(() => void api.appReady());
    });
    const settings = listen("open-settings", () => useDialogs.getState().open({ kind: "settings" }));
    const update = listen<string>("update-ready", (event) =>
      toast(`Redrule ${event.payload} is ready`, {
        description: "Restart to start using it.",
        duration: Infinity,
        action: { label: "Restart", onClick: () => void api.restartToUpdate() },
      }),
    );
    return () => {
      cancelled = true;
      disconnect?.();
      void settings.then((stop) => stop());
      void update.then((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (storageError) toast.error(storageError, { duration: Infinity });
  }, [storageError]);

  return (
    <div className="flex h-full">
      <Sidebar />
      <MeetingPane />
      <DialogHost />
      <Toaster
        position="bottom-right"
        toastOptions={{
          className: "!rounded-[10px] !border-rule !bg-raised !text-ink !shadow-float !text-[13px]",
        }}
      />
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
