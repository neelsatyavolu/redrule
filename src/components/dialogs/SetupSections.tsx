import clsx from "clsx";
import { Check } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import { LOCAL_NOTES, writesNotesLocally, type ProviderId } from "../../lib/types";
import { Button, TextInput } from "../ui";
import { ModelStatus, sizeLabel, useLocalModels } from "./TranscriptionSettings";

const PROVIDERS: { id: ProviderId; name: string; detail: string }[] = [
  { id: "codex", name: "ChatGPT", detail: "Writes notes with your ChatGPT plan, through Codex." },
  { id: "grok", name: "Grok", detail: "Writes notes with your Grok account." },
];

function StatusMark({ done }: { done: boolean }) {
  return (
    <span
      aria-label={done ? "Done" : "Not done"}
      className={clsx(
        "mt-0.5 grid size-[18px] shrink-0 place-items-center rounded-full",
        done ? "bg-focus text-white" : "border-[1.5px] border-faint",
      )}
    >
      {done && <Check size={11} strokeWidth={3} />}
    </span>
  );
}

export function SetupRow({ title, detail, done, action, children }: { title: string; detail: string; done: boolean; action?: ReactNode; children?: ReactNode }) {
  return (
    <div className="py-3.5">
      <div className="flex items-start gap-3">
        <StatusMark done={done} />
        <div className="min-w-0 flex-1">
          <div className="text-[13px] font-medium text-ink">{title}</div>
          <div className="mt-0.5 text-[12px] leading-relaxed text-graphite">{detail}</div>
        </div>
        {action && <div className="shrink-0">{action}</div>}
      </div>
      {children && <div className="mt-3 pl-[30px]">{children}</div>}
    </div>
  );
}

export function PermissionRows() {
  const permissions = useStore((s) => s.app?.permissions);
  if (!permissions) return null;
  return (
    <div className="divide-y divide-rule">
      <SetupRow
        title="Microphone"
        detail="So your side of the conversation is in the notes."
        done={permissions.microphone}
        action={!permissions.microphone && <Button size="sm" onClick={() => void attempt(api.requestMicrophone)}>Allow</Button>}
      />
      <SetupRow
        title="Screen & System Audio Recording"
        detail="macOS files call audio under this permission. Redrule records sound only, never the screen. Quit and reopen Redrule after switching it on."
        done={permissions.screenRecording}
        action={
          !permissions.screenRecording && <Button size="sm" onClick={() => void attempt(api.requestScreenRecording)}>Allow</Button>
        }
      />
    </div>
  );
}

export function AccountRows() {
  const app = useStore((s) => s.app);
  const [pasted, setPasted] = useState("");
  if (!app) return null;

  return (
    <div>
      <div className="divide-y divide-rule">
        {PROVIDERS.map((provider) => {
          const connected = app.connected.includes(provider.id);
          const connecting = app.connecting === provider.id;
          return (
            <SetupRow
              key={provider.id}
              title={provider.name}
              detail={provider.detail}
              done={connected}
              action={
                connected ? (
                  <Button size="sm" variant="ghost" onClick={() => void attempt(() => api.disconnect(provider.id))}>
                    Disconnect
                  </Button>
                ) : connecting ? (
                  <Button size="sm" variant="ghost" onClick={() => void attempt(api.cancelConnecting)}>
                    Cancel
                  </Button>
                ) : (
                  <Button size="sm" disabled={app.connecting !== null} onClick={() => void attempt(() => api.connect(provider.id))}>
                    Connect
                  </Button>
                )
              }
            >
              {connecting && (
                <form
                  onSubmit={(event) => {
                    event.preventDefault();
                    void attempt(() => api.submitPastedCode(pasted));
                    setPasted("");
                  }}
                >
                  <p className="text-[12px] leading-relaxed text-graphite">
                    Finish signing in in your browser. If it ends on a page that won’t load, paste that page’s address here.
                  </p>
                  <div className="mt-2 flex gap-2">
                    <TextInput
                      value={pasted}
                      onChange={(e) => setPasted(e.target.value)}
                      placeholder="Code or address"
                      aria-label="Code or address"
                    />
                    <Button size="md" type="submit" disabled={pasted.trim() === ""}>
                      Finish connecting
                    </Button>
                  </div>
                </form>
              )}
            </SetupRow>
          );
        })}
      </div>
      {app.connectionError && <p className="mt-1 text-[12px] text-margin">{app.connectionError}</p>}
    </div>
  );
}

/** Writing notes on this Mac instead of connecting an account. */
export function LocalNotesRow() {
  const app = useStore((s) => s.app);
  const [models] = useLocalModels();
  const model = models?.notes.find((m) => m.recommended);
  if (!app || !model) return null;
  const chosen = writesNotesLocally(app.settings);
  const size = model.installed ? "" : ` It is a one-time ${sizeLabel(model.sizeMb)} download.`;

  return (
    <SetupRow
      title="Or write notes on this Mac"
      detail={`No account needed, and the transcript never leaves this Mac. Notes take a minute or two longer.${size}`}
      done={chosen}
      action={
        !chosen && (
          <Button size="sm" onClick={() => void attempt(() => api.updateSettings({ modelChoiceId: LOCAL_NOTES + model.id }))}>
            Use
          </Button>
        )
      }
    >
      {chosen && app.noteModel && <ModelStatus model={app.noteModel} onRetry={api.retryNoteModel} />}
    </SetupRow>
  );
}
