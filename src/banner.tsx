import { listen } from "@tauri-apps/api/event";
import { StrictMode, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { Button } from "./components/ui";
import { api } from "./lib/api";
import { APP_NAMES } from "./lib/format";
import type { AppState, Banner } from "./lib/types";
import "./styles.css";

/** The floating call prompt. It lives in its own non-activating panel, so it never takes focus from the call. */
function CallBanner() {
  const [banner, setBanner] = useState<Banner | null>(null);

  useEffect(() => {
    const stop = listen<AppState>("state", (event) => setBanner(event.payload.banner));
    void api.getState().then((state) => setBanner(state.banner));
    return () => void stop.then((unlisten) => unlisten());
  }, []);

  if (!banner) return null;
  const detected = banner.kind === "detected";

  return (
    <div className="p-2">
      <div className="rise flex gap-3.5 rounded-[14px] border border-rule bg-paper p-4 shadow-float">
        <span aria-hidden className="w-[2px] shrink-0 rounded-full bg-margin" />
        <div className="min-w-0 flex-1">
          <p className="font-serif text-[16.5px] leading-snug font-semibold text-ink">
            {detected ? `You're in a ${APP_NAMES[banner.app]} call` : "The call looks finished"}
          </p>
          <p className="mt-0.5 text-[12px] text-graphite">
            {detected ? "Redrule can record it and write up the notes." : "Stop recording and write the notes now?"}
          </p>
          <div className="mt-3 flex gap-2">
            <Button
              size="sm"
              variant="record"
              onClick={() => void (detected ? api.startRecording(banner.app) : api.stopRecording())}
            >
              {detected ? "Record" : "Stop and write notes"}
            </Button>
            <Button size="sm" variant="quiet" onClick={() => void api.dismissBanner()}>
              {detected ? "Not this one" : "Keep recording"}
            </Button>
          </div>
        </div>
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <CallBanner />
  </StrictMode>,
);
