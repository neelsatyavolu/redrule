import clsx from "clsx";
import { ArrowUp, Sparkles, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { useAsk, useThread } from "../lib/ask";
import { useDialogs } from "./dialogs/dialogState";
import { Spinner } from "./ui";

const SUGGESTIONS = ["What did we decide?", "What do I need to do next?", "What was left open?"];

/**
 * A quiet "Ask about this meeting" field floating at the bottom of a finished meeting. It opens
 * into a short conversation above the field; Escape, the close button or a click elsewhere folds
 * it back. ⌘J jumps to it from anywhere in the meeting.
 */
export function AskBar({ meetingId }: { meetingId: string }) {
  const { exchanges, pending } = useThread(meetingId);
  const ask = useAsk((s) => s.ask);
  const clear = useAsk((s) => s.clear);
  const [open, setOpen] = useState(false);
  const [draft, setDraft] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const log = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.metaKey && event.key.toLowerCase() === "j" && !useDialogs.getState().current) {
        event.preventDefault();
        setOpen(true);
        input.current?.focus();
      }
    };
    const onPointer = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("pointerdown", onPointer);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onPointer);
    };
  }, []);

  useEffect(() => {
    log.current?.scrollTo({ top: log.current.scrollHeight });
  }, [exchanges.length, pending, open]);

  const submit = async (question: string) => {
    const text = question.trim();
    if (!text || pending) return;
    setOpen(true);
    setDraft("");
    if (!(await ask(meetingId, text))) setDraft(text);
    input.current?.focus();
  };

  const hasThread = exchanges.length > 0 || pending !== null;

  return (
    <div className="pointer-events-none absolute inset-x-0 bottom-4 z-10 flex justify-center px-6">
      <div
        ref={root}
        className={clsx(
          "pointer-events-auto flex max-w-full flex-col overflow-hidden rounded-[12px] border border-rule bg-raised has-[input:focus]:border-focus/60 shadow-float transition-[width] duration-200 ease-out",
          open ? "w-[600px]" : "w-[340px]",
        )}
      >
        {open && (
          <div className="arrive border-b border-rule">
            {hasThread ? (
              <>
                <div ref={log} className="max-h-[min(52vh,420px)] space-y-5 overflow-y-auto px-4 pt-4 pb-4 selectable">
                  {exchanges.map((exchange, index) => (
                    <Turn key={index} question={exchange.question}>
                      <Answer text={exchange.answer} />
                    </Turn>
                  ))}
                  {pending && (
                    <Turn question={pending}>
                      <span className="flex items-center gap-2 text-[13px] text-graphite">
                        <Spinner /> Reading the meeting…
                      </span>
                    </Turn>
                  )}
                </div>
                <div className="flex items-center justify-end gap-1 px-2 pb-1.5">
                  <button
                    type="button"
                    disabled={pending !== null}
                    onClick={() => clear(meetingId)}
                    className="h-6 rounded-[6px] px-2 text-[12px] text-graphite hover:bg-wash hover:text-ink disabled:opacity-40"
                  >
                    Clear
                  </button>
                  <button
                    type="button"
                    aria-label="Close"
                    title="Close (Esc)"
                    onClick={() => setOpen(false)}
                    className="grid size-6 place-items-center rounded-[6px] text-graphite hover:bg-wash hover:text-ink"
                  >
                    <X size={13} />
                  </button>
                </div>
              </>
            ) : (
              <div className="px-4 py-3.5">
                <p className="text-[12px] text-graphite">Answers come from this meeting's notes and transcript.</p>
                <div className="mt-2.5 flex flex-wrap gap-1.5">
                  {SUGGESTIONS.map((suggestion) => (
                    <button
                      key={suggestion}
                      type="button"
                      onClick={() => void submit(suggestion)}
                      className="h-7 rounded-full bg-wash px-3 text-[12.5px] text-ink hover:bg-rule"
                    >
                      {suggestion}
                    </button>
                  ))}
                </div>
              </div>
            )}
          </div>
        )}

        <form
          className="flex h-10 items-center gap-2 pr-1.5 pl-3"
          onSubmit={(event) => {
            event.preventDefault();
            void submit(draft);
          }}
        >
          <Sparkles size={14} className="shrink-0 text-focus" aria-hidden />
          <input
            ref={input}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onFocus={() => setOpen(true)}
            onKeyDown={(event) => {
              if (event.key === "Escape") {
                setOpen(false);
                input.current?.blur();
              }
            }}
            maxLength={2000}
            aria-label="Ask about this meeting"
            placeholder={exchanges.length > 0 ? "Ask a follow-up" : "Ask about this meeting"}
            className="min-w-0 flex-1 bg-transparent text-[13px] text-ink placeholder:text-graphite focus-visible:outline-none!"
          />
          {pending && !open ? (
            <Spinner className="mr-2" />
          ) : open ? (
            <button
              type="submit"
              aria-label="Ask"
              disabled={!draft.trim() || pending !== null}
              className="grid size-7 shrink-0 place-items-center rounded-[7px] bg-ink text-paper transition-opacity disabled:opacity-25"
            >
              <ArrowUp size={14} strokeWidth={2.5} />
            </button>
          ) : (
            <kbd className="mr-1.5 font-sans text-[11px] text-faint">⌘J</kbd>
          )}
        </form>
      </div>
    </div>
  );
}

function Turn({ question, children }: { question: string; children: React.ReactNode }) {
  return (
    <div>
      <p className="text-[12.5px] font-medium text-graphite">{question}</p>
      <div className="mt-1">{children}</div>
    </div>
  );
}

/** Plain-text answers: paragraphs, with lines starting "- " set as a list. */
function Answer({ text }: { text: string }) {
  const blocks = text.split(/\n\s*\n/);
  return (
    <div className="space-y-2 text-[14px] leading-[1.6] text-ink">
      {blocks.map((block, index) => {
        const lines = block.split("\n").filter((line) => line.trim());
        if (lines.length > 0 && lines.every((line) => /^\s*[-•*]\s/.test(line))) {
          return (
            <ul key={index} className="space-y-1">
              {lines.map((line, lineIndex) => (
                <li key={lineIndex} className="flex gap-2.5">
                  <span aria-hidden className="text-faint">
                    –
                  </span>
                  <span>{line.replace(/^\s*[-•*]\s/, "")}</span>
                </li>
              ))}
            </ul>
          );
        }
        return (
          <p key={index} className="whitespace-pre-line">
            {block}
          </p>
        );
      })}
    </div>
  );
}
