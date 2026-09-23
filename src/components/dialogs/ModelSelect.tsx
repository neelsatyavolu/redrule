import { useState, type FormEvent } from "react";
import { LOCAL_NOTES, type ApiProvider, type LocalModel, type ModelOption, type NoteProvider } from "../../lib/types";
import { Button, TextInput } from "../ui";
import { sizeLabel } from "./TranscriptionSettings";

export const SELECT =
  "h-8 w-[220px] rounded-[7px] border border-rule bg-raised px-2 text-[13px] text-ink focus:border-focus focus:outline-none";

const PROVIDER_NAMES: Record<NoteProvider, string> = {
  codex: "ChatGPT",
  grok: "Grok",
  openai: "OpenAI",
  anthropic: "Anthropic",
  gemini: "Gemini",
  compatible: "Custom server",
};
/** API providers with listed models, where a model name can also be typed in. */
const KEYED: ApiProvider[] = ["openai", "anthropic", "gemini"];
/** Select value that asks for a typed model name, followed by the provider. */
const OTHER = "other:";

const providerOf = (id: string) => id.split(":")[0] as NoteProvider;

/** Where an account or API model runs, for the help under a model picker. */
export function remoteDetail(id: string): string {
  const provider = providerOf(id);
  if (provider === "compatible") return "Uses the server set up under Accounts.";
  if ((KEYED as string[]).includes(provider)) {
    return `Uses your ${PROVIDER_NAMES[provider]} API key, billed by ${PROVIDER_NAMES[provider]}.`;
  }
  return "Uses the account connected under Accounts.";
}

interface ModelSelectProps {
  label: string;
  value: string;
  onChange: (id: string) => void;
  models: ModelOption[];
  localModels: LocalModel[];
  /** Providers set up under Accounts; API providers are listed only once they are. */
  connected: NoteProvider[];
  /** The custom server's model name. */
  serverModel: string;
  /** Offers following the notes model, stored as an empty choice. */
  sameAsNotes?: boolean;
}

/** Account models by provider, API key models, then the models that run on this Mac. */
export function ModelSelect({ label, value, onChange, models, localModels, connected, serverModel, sameAsNotes }: ModelSelectProps) {
  const [typing, setTyping] = useState<ApiProvider | null>(null);
  const [typed, setTyped] = useState("");
  const current = providerOf(value);
  // The saved choice stays visible even after its key is removed.
  const shown = (provider: NoteProvider) => connected.includes(provider) || current === provider;
  const serverChoice = `compatible:${serverModel}`;

  const choose = (next: string) => {
    if (next.startsWith(OTHER)) {
      setTyping(next.slice(OTHER.length) as ApiProvider);
      setTyped("");
      return;
    }
    setTyping(null);
    onChange(next);
  };
  const submitTyped = (event: FormEvent) => {
    event.preventDefault();
    if (typing) onChange(`${typing}:${typed.trim()}`);
    setTyping(null);
  };

  return (
    <div className="flex flex-col items-end gap-1.5">
      <select aria-label={label} className={SELECT} value={typing ? OTHER + typing : value} onChange={(e) => choose(e.target.value)}>
        {sameAsNotes && <option value="">Same as notes</option>}
        {(["codex", "grok"] as const).map((provider) => (
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
        {KEYED.filter(shown).map((provider) => {
          const listed = models.filter((m) => m.provider === provider);
          const typedChoice = current === provider && !listed.some((m) => m.id === value);
          return (
            <optgroup key={provider} label={PROVIDER_NAMES[provider]}>
              {listed.map((model) => (
                <option key={model.id} value={model.id}>
                  {model.label}
                </option>
              ))}
              {typedChoice && <option value={value}>{value.slice(provider.length + 1)}</option>}
              <option value={OTHER + provider}>Other model…</option>
            </optgroup>
          );
        })}
        {shown("compatible") && (
          <optgroup label={PROVIDER_NAMES.compatible}>
            {current === "compatible" && value !== serverChoice && <option value={value}>{value.slice("compatible:".length)}</option>}
            {serverModel && <option value={serverChoice}>{serverModel}</option>}
          </optgroup>
        )}
        <optgroup label="On this Mac">
          {localModels.map((model) => (
            <option key={model.id} value={LOCAL_NOTES + model.id}>
              {model.installed ? model.name : `${model.name} (${sizeLabel(model.sizeMb)} download)`}
            </option>
          ))}
        </optgroup>
      </select>
      {typing && (
        <form onSubmit={submitTyped} className="flex w-[220px] gap-1.5">
          <TextInput
            autoFocus
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            placeholder="Model name"
            aria-label={`${PROVIDER_NAMES[typing]} model name`}
            spellCheck={false}
            autoCapitalize="off"
          />
          <Button size="md" type="submit" disabled={typed.trim() === ""}>
            Use
          </Button>
        </form>
      )}
    </div>
  );
}
