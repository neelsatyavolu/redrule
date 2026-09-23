import { useCallback, useEffect, useState } from "react";
import { api } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import { LOCAL_NOTES, type LocalModel, type LocalModels, type ModelKind, type Settings, type SpeechModel } from "../../lib/types";
import { Button, Spinner } from "../ui";

const SETTING: Record<ModelKind, (id: string) => Partial<Settings>> = {
  speech: (id) => ({ speechModelId: id }),
  speaker: (id) => ({ speakerModelId: id }),
  notes: (id) => ({ modelChoiceId: LOCAL_NOTES + id }),
};

/** The on-device models, refreshed when a download finishes so "Downloaded" stays true. */
export function useLocalModels(): [LocalModels | null, () => void] {
  const [models, setModels] = useState<LocalModels | null>(null);
  const reload = useCallback(() => void api.localModels().then(setModels), []);
  const speech = useStore((s) => s.app?.speechModel.state);
  const notes = useStore((s) => s.app?.noteModel?.state);
  useEffect(reload, [reload, speech, notes]);
  return [models, reload];
}

/** Choosing the on-device speech, speaker and note models. */
export function TranscriptionSettings() {
  const app = useStore((s) => s.app);
  const [models, reload] = useLocalModels();
  if (!app || !models) return null;

  const { settings, speechModel, noteModel } = app;
  const recording = app.recordingId !== null;
  const localNotes = settings.modelChoiceId.startsWith(LOCAL_NOTES) ? settings.modelChoiceId.slice(LOCAL_NOTES.length) : "";
  const choose = (kind: ModelKind, id: string) => void attempt(() => api.updateSettings(SETTING[kind](id)));
  const remove = (kind: ModelKind, model: LocalModel) =>
    void attempt(() => api.removeLocalModel(kind, model.id), `${model.name} removed.`).then(reload);

  return (
    <div className="space-y-5">
      <p className="text-[12px] leading-relaxed text-graphite">
        These run on this Mac, so nothing you say leaves it until notes are written. Recommendations suit this Mac (
        {describeHardware(models.hardware)}) and stay light enough to run alongside a call.
        {recording && " Changes apply to the next recording."}
      </p>
      <ModelGroup
        title="Speech recognition"
        detail="Turns speech into text."
        kind="speech"
        models={models.speech}
        selected={settings.speechModelId}
        onChoose={choose}
        onRemove={remove}
      />
      <ModelGroup
        title="Speaker recognition"
        detail="Tells the other people on the call apart. Your microphone is always labelled as you."
        kind="speaker"
        models={models.speaker}
        selected={settings.speakerModelId}
        onChoose={choose}
        onRemove={remove}
      />
      <ModelStatus model={speechModel} onRetry={api.retrySpeechModel} />
      <ModelGroup
        title="Notes"
        detail="Writes the notes on this Mac instead of with ChatGPT or Grok, so the transcript never leaves it. Slower than an account, and free."
        kind="notes"
        models={models.notes}
        selected={localNotes}
        onChoose={choose}
        onRemove={remove}
      />
      {noteModel && <ModelStatus model={noteModel} onRetry={api.retryNoteModel} />}
    </div>
  );
}

interface GroupProps {
  title: string;
  detail: string;
  kind: ModelKind;
  models: LocalModel[];
  selected: string;
  onChoose: (kind: ModelKind, id: string) => void;
  onRemove: (kind: ModelKind, model: LocalModel) => void;
}

function ModelGroup({ title, detail, kind, models, selected, onChoose, onRemove }: GroupProps) {
  return (
    <fieldset>
      <legend className="text-[13px] font-medium text-ink">{title}</legend>
      <p className="mt-0.5 mb-2 text-[12px] text-graphite">{detail}</p>
      <div className="divide-y divide-rule rounded-[8px] border border-rule">
        {models.map((model) => (
          <ModelRow
            key={model.id}
            model={model}
            name={`${kind}-model`}
            selected={model.id === selected}
            onChoose={() => onChoose(kind, model.id)}
            onRemove={() => onRemove(kind, model)}
          />
        ))}
      </div>
    </fieldset>
  );
}

interface RowProps {
  model: LocalModel;
  name: string;
  selected: boolean;
  onChoose: () => void;
  onRemove: () => void;
}

function ModelRow({ model, name, selected, onChoose, onRemove }: RowProps) {
  const facts = [
    sizeLabel(model.sizeMb),
    model.languages,
    selected ? null : model.installed ? "Downloaded" : "Downloads when chosen",
  ].filter(Boolean);
  return (
    <div className="flex items-start gap-3 px-3 py-2.5">
      <input
        type="radio"
        id={`${name}-${model.id}`}
        name={name}
        checked={selected}
        onChange={onChoose}
        className="mt-[3px] accent-[var(--focus)]"
      />
      <label htmlFor={`${name}-${model.id}`} className="min-w-0 flex-1 cursor-default">
        <span className="flex items-center gap-2 text-[13px] text-ink">
          {model.name}
          {model.recommended && (
            <span className="rounded-full bg-wash px-1.5 py-px text-[10.5px] font-medium text-graphite">
              Recommended
            </span>
          )}
        </span>
        <span className="mt-0.5 block text-[12px] leading-relaxed text-graphite">{model.description}</span>
        <span className="mt-0.5 block text-[11.5px] text-faint">{facts.join(" · ")}</span>
      </label>
      {model.installed && !selected && (
        <Button size="sm" onClick={onRemove}>
          Remove
        </Button>
      )}
    </div>
  );
}

export function ModelStatus({ model, onRetry }: { model: SpeechModel; onRetry: () => Promise<void> }) {
  if (model.state === "ready") return <p className="text-[12px] text-graphite">Ready.</p>;
  if (model.state === "failed") {
    return (
      <div className="flex items-center gap-3">
        <p className="flex-1 text-[12px] text-margin">{model.message}</p>
        <Button size="sm" onClick={() => void attempt(onRetry)}>
          Retry
        </Button>
      </div>
    );
  }
  const progress = model.progress;
  const percent =
    progress?.totalBytes ? ` ${Math.round((progress.downloadedBytes / progress.totalBytes) * 100)}%` : "";
  return (
    <p className="flex items-center gap-2 text-[12px] text-graphite">
      <Spinner className="size-3" />
      {progress ? `${progress.stage}${percent}` : "Preparing the models"}
    </p>
  );
}

function describeHardware({ memoryGb, appleSilicon }: LocalModels["hardware"]): string {
  return `${appleSilicon ? "Apple silicon" : "Intel"}, ${memoryGb} GB memory`;
}

export function sizeLabel(mb: number): string {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}
