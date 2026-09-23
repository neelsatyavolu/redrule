import { useState, type FormEvent } from "react";
import { api, errorMessage } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import type { ApiProvider, Settings } from "../../lib/types";
import { Button, TextInput } from "../ui";
import { SetupRow } from "./SetupSections";

interface ApiProviderInfo {
  id: ApiProvider;
  name: string;
  detail: string;
  placeholder: string;
}

export const API_PROVIDERS: ApiProviderInfo[] = [
  { id: "openai", name: "OpenAI", detail: "Use your OpenAI API key. OpenAI bills you for what you use.", placeholder: "sk-…" },
  { id: "anthropic", name: "Anthropic", detail: "Use your Claude API key. Anthropic bills you for what you use.", placeholder: "sk-ant-…" },
  { id: "gemini", name: "Google Gemini", detail: "Use your Gemini API key. Google bills you for what you use.", placeholder: "AIza…" },
  {
    id: "compatible",
    name: "OpenAI-compatible server",
    detail: "OpenRouter, Groq, xAI, or a model on this Mac with Ollama or LM Studio.",
    placeholder: "API key, if the server needs one",
  },
];

function savedDetail(provider: ApiProvider, settings: Settings): string {
  if (provider !== "compatible") return "Key saved in your Keychain.";
  return `${settings.compatibleModel} at ${settings.compatibleUrl}`;
}

/** A row per provider: paste a key, see that it is saved, remove it. */
export function ApiKeyRows() {
  const app = useStore((s) => s.app);
  const [editing, setEditing] = useState<ApiProvider | null>(null);
  if (!app) return null;

  return (
    <div className="divide-y divide-rule">
      {API_PROVIDERS.map((provider) => {
        const saved = app.connected.includes(provider.id);
        const open = editing === provider.id && !saved;
        return (
          <SetupRow
            key={provider.id}
            title={provider.name}
            detail={saved ? savedDetail(provider.id, app.settings) : provider.detail}
            done={saved}
            action={
              saved ? (
                <Button size="sm" variant="ghost" onClick={() => void attempt(() => api.removeApiKey(provider.id))}>
                  Remove
                </Button>
              ) : open ? (
                <Button size="sm" variant="ghost" onClick={() => setEditing(null)}>
                  Cancel
                </Button>
              ) : (
                <Button size="sm" onClick={() => setEditing(provider.id)}>
                  {provider.id === "compatible" ? "Set up" : "Add key"}
                </Button>
              )
            }
          >
            {open && <KeyForm provider={provider} onSaved={() => setEditing(null)} />}
          </SetupRow>
        );
      })}
    </div>
  );
}

function KeyForm({ provider, onSaved }: { provider: ApiProviderInfo; onSaved: () => void }) {
  const [key, setKey] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [model, setModel] = useState("");
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const server = provider.id === "compatible";
  const ready = server ? baseUrl.trim() !== "" && model.trim() !== "" : key.trim() !== "";

  const save = async (event: FormEvent) => {
    event.preventDefault();
    setChecking(true);
    setError(null);
    try {
      await api.saveApiKey(provider.id, key, baseUrl, model);
      onSaved();
    } catch (failure) {
      setError(errorMessage(failure));
      setChecking(false);
    }
  };

  return (
    <form onSubmit={(event) => void save(event)} className="space-y-2">
      {server && (
        <>
          <TextInput
            autoFocus
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://openrouter.ai/api/v1"
            aria-label="Server address"
            spellCheck={false}
            autoCapitalize="off"
          />
          <TextInput
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="Model name, like llama3.2"
            aria-label="Model name"
            spellCheck={false}
            autoCapitalize="off"
          />
        </>
      )}
      <div className="flex gap-2">
        <TextInput
          autoFocus={!server}
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder={provider.placeholder}
          aria-label={`${provider.name} API key`}
        />
        <Button size="md" type="submit" disabled={!ready || checking}>
          {checking ? "Checking…" : "Save"}
        </Button>
      </div>
      {error ? (
        <p role="alert" className="text-[12px] leading-relaxed text-margin">
          {error}
        </p>
      ) : (
        <p className="text-[12px] leading-relaxed text-graphite">
          {server
            ? "Redrule checks that the server answers before saving. Plain http:// works only for localhost."
            : `Redrule checks the key with ${provider.name}, then keeps it in your Keychain.`}
        </p>
      )}
    </form>
  );
}

/** First launch: a way to pick an API key instead of an account, folded away until asked for. */
export function ApiKeyChoiceRow() {
  const connected = useStore((s) => s.app?.connected);
  const [open, setOpen] = useState(false);
  const saved = API_PROVIDERS.filter((p) => connected?.includes(p.id));

  return (
    <SetupRow
      title="Or use an API key"
      detail={
        saved.length > 0
          ? `Using ${saved.map((p) => p.name).join(", ")}.`
          : "OpenAI, Anthropic, Gemini, or an OpenAI-compatible server. You pay the provider for what you use."
      }
      done={saved.length > 0}
      action={
        <Button size="sm" variant={open ? "ghost" : "quiet"} onClick={() => setOpen(!open)}>
          {open ? "Hide" : "Choose"}
        </Button>
      }
    >
      {open && <ApiKeyRows />}
    </SetupRow>
  );
}
