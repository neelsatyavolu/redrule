import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import {
  LOCAL_NOTES,
  writesNotesLocally,
  type LocalModel,
  type Microphone,
  type ModelOption,
  type ProviderId,
} from "../../lib/types";
import { Dialog, Segmented, SettingRow, Switch } from "../ui";
import type { SettingsTab } from "./dialogState";
import { AccountRows, PermissionRows } from "./SetupSections";
import { ModelStatus, sizeLabel, TranscriptionSettings, useLocalModels } from "./TranscriptionSettings";

const PROVIDER_NAMES: Record<ProviderId, string> = { codex: "ChatGPT", grok: "Grok" };

export function SettingsDialog({ initialTab = "general", onClose }: { initialTab?: SettingsTab; onClose: () => void }) {
  const [tab, setTab] = useState<SettingsTab>(initialTab);
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} title="Settings" width={560}>
      <Segmented
        label="Settings section"
        className="mb-4"
        value={tab}
        onChange={setTab}
        options={[
          { value: "general", label: "General" },
          { value: "transcription", label: "Models" },
          { value: "accounts", label: "Accounts" },
          { value: "permissions", label: "Permissions" },
        ]}
      />
      <div className="min-h-[300px]">
        {tab === "general" && <GeneralSettings />}
        {tab === "transcription" && <TranscriptionSettings />}
        {tab === "accounts" && <AccountRows />}
        {tab === "permissions" && <PermissionRows />}
      </div>
    </Dialog>
  );
}

const SELECT =
  "h-8 w-[220px] rounded-[7px] border border-rule bg-raised px-2 text-[13px] text-ink focus:border-focus focus:outline-none";

function GeneralSettings() {
  const app = useStore((s) => s.app);
  const microphones = useMicrophones();
  const [models, setModels] = useState<ModelOption[]>([]);
  const [localModels] = useLocalModels();
  useEffect(() => void api.modelChoices().then(setModels), []);
  if (!app) return null;
  const { settings } = app;
  const update = (patch: Parameters<typeof api.updateSettings>[0]) => void attempt(() => api.updateSettings(patch));
  const missingMicrophone = settings.microphoneId !== "" && !microphones.some((m) => m.id === settings.microphoneId);

  return (
    <div className="divide-y divide-rule">
      <SettingRow title="Microphone" detail="Changes apply to the next recording.">
        <select
          aria-label="Microphone"
          className={SELECT}
          value={settings.microphoneId}
          onChange={(e) => update({ microphoneId: e.target.value })}
        >
          <option value="">System default</option>
          {microphones.map((microphone) => (
            <option key={microphone.id} value={microphone.id}>
              {microphone.name}
            </option>
          ))}
          {missingMicrophone && <option value={settings.microphoneId}>Selected microphone (disconnected)</option>}
        </select>
      </SettingRow>

      <SettingRow
        title="Write notes with"
        detail={
          writesNotesLocally(settings) ? (
            <>
              Runs on this Mac, so the transcript never leaves it. Takes a minute or two after each meeting.
              {app.noteModel && app.noteModel.state !== "ready" && (
                <div className="mt-1.5">
                  <ModelStatus model={app.noteModel} onRetry={api.retryNoteModel} />
                </div>
              )}
            </>
          ) : (
            "Uses the account connected under Accounts."
          )
        }
      >
        <ModelSelect
          label="Model"
          value={settings.modelChoiceId}
          onChange={(modelChoiceId) => update({ modelChoiceId })}
          models={models}
          localModels={localModels?.notes ?? []}
        />
      </SettingRow>

      <SettingRow
        title="Answer questions with"
        detail={
          settings.askModelChoiceId === "" ? (
            "Asks the model that writes the notes."
          ) : settings.askModelChoiceId.startsWith(LOCAL_NOTES) ? (
            <>
              Runs on this Mac, so the meeting never leaves it. Slower than an account.
              {app.askModel && app.askModel.state !== "ready" && (
                <div className="mt-1.5">
                  <ModelStatus model={app.askModel} onRetry={api.retryNoteModel} />
                </div>
              )}
            </>
          ) : (
            "Uses the account connected under Accounts."
          )
        }
      >
        <ModelSelect
          label="Model for questions"
          value={settings.askModelChoiceId}
          onChange={(askModelChoiceId) => update({ askModelChoiceId })}
          models={models}
          localModels={localModels?.notes ?? []}
          sameAsNotes
        />
      </SettingRow>

      <SettingRow
        title="Keep audio recordings"
        detail="Off by default: audio is transcribed as the meeting runs and never written to disk. Turn this on to keep a WAV file of each side next to the notes."
      >
        <Switch label="Keep audio recordings" checked={settings.keepAudio} onChange={(keepAudio) => update({ keepAudio })} />
      </SettingRow>
    </div>
  );
}

interface ModelSelectProps {
  label: string;
  value: string;
  onChange: (id: string) => void;
  models: ModelOption[];
  localModels: LocalModel[];
  /** Offers following the notes model, stored as an empty choice. */
  sameAsNotes?: boolean;
}

/** Account models by provider, then the models that run on this Mac. */
function ModelSelect({ label, value, onChange, models, localModels, sameAsNotes }: ModelSelectProps) {
  return (
    <select aria-label={label} className={SELECT} value={value} onChange={(e) => onChange(e.target.value)}>
      {sameAsNotes && <option value="">Same as notes</option>}
      {(["codex", "grok"] as ProviderId[]).map((provider) => (
        <optgroup key={provider} label={PROVIDER_NAMES[provider]}>
          {models
            .filter((m) => m.provider === provider)
            .map((model) => (
              <option key={model.id} value={model.id}>
                {model.label}
              </option>
            ))}
        </optgroup>
      ))}
      <optgroup label="On this Mac">
        {localModels.map((model) => (
          <option key={model.id} value={LOCAL_NOTES + model.id}>
            {model.installed ? model.name : `${model.name} (${sizeLabel(model.sizeMb)} download)`}
          </option>
        ))}
      </optgroup>
    </select>
  );
}

/** Input devices, refreshed while settings are open so a newly plugged-in headset appears. */
function useMicrophones(): Microphone[] {
  const [microphones, setMicrophones] = useState<Microphone[]>([]);
  useEffect(() => {
    let current = true;
    const load = () => api.microphones().then((list) => current && setMicrophones(list));
    void load();
    const timer = window.setInterval(load, 2000);
    return () => {
      current = false;
      window.clearInterval(timer);
    };
  }, []);
  return microphones;
}
