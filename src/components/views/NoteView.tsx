import { Check } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "../../lib/api";
import { meetingSentence, shortDate } from "../../lib/format";
import { attempt } from "../../lib/store";
import type { ActionItem, Meeting, MeetingNote } from "../../lib/types";
import { PadPage, PadRow } from "../Pad";

/** The finished notes, set on the pad. Action item owners hang in the margin. */
export function NoteView({ meeting, note }: { meeting: Meeting; note: MeetingNote }) {
  const [actionItems, toggle] = useCheckedItems(meeting.id, note);
  return (
    <PadPage>
      <article className="selectable">
        <MeetingHeading meeting={meeting} title={note.title} />
        {note.tldr && (
          <PadRow className="mt-7">
            <p className="font-serif text-[17px] leading-[1.6] text-ink">{note.tldr}</p>
          </PadRow>
        )}

        {note.sections.map((section, index) => (
          <section key={`${index}-${section.heading}`} className="mt-9">
            <PadRow>
              <SectionHeading>{section.heading}</SectionHeading>
            </PadRow>
            <ul className="mt-2 space-y-1.5">
              {section.bullets.map((bullet, bulletIndex) => (
                <PadRow key={bulletIndex}>
                  <li className="flex gap-3 text-[14.5px] leading-[1.6]">
                    <span aria-hidden className="text-faint">
                      –
                    </span>
                    <span>{bullet}</span>
                  </li>
                </PadRow>
              ))}
            </ul>
          </section>
        ))}

        {note.decisions.length > 0 && (
          <section className="mt-9">
            <PadRow>
              <SectionHeading>Decisions</SectionHeading>
            </PadRow>
            <ul className="mt-2 space-y-1.5">
              {note.decisions.map((decision, index) => (
                <PadRow key={index} label={<Check size={13} strokeWidth={2.5} className="ml-auto inline text-focus" aria-label="Decided" />}>
                  <li className="text-[14.5px] leading-[1.6]">{decision}</li>
                </PadRow>
              ))}
            </ul>
          </section>
        )}

        {actionItems.length > 0 && (
          <section className="mt-9">
            <PadRow>
              <SectionHeading>Action items</SectionHeading>
            </PadRow>
            <ul className="mt-2 space-y-2">
              {actionItems.map((item, index) => (
                <PadRow key={index} label={<span className="font-medium text-ink">{item.owner}</span>}>
                  <li className="flex gap-3 text-[14.5px] leading-[1.6]">
                    <button
                      type="button"
                      role="checkbox"
                      aria-checked={!!item.done}
                      aria-label={item.done ? "Mark as not done" : "Mark as done"}
                      onClick={() => toggle(index)}
                      className={`mt-[5px] grid size-3.5 shrink-0 cursor-default place-items-center rounded-[4px] border-[1.5px] transition-colors ${
                        item.done ? "border-focus bg-focus text-paper" : "border-faint hover:border-graphite"
                      }`}
                    >
                      {item.done && <Check size={10} strokeWidth={3.5} aria-hidden />}
                    </button>
                    <span className={item.done ? "text-graphite line-through decoration-faint" : undefined}>{item.task}</span>
                  </li>
                </PadRow>
              ))}
            </ul>
          </section>
        )}
      </article>
    </PadPage>
  );
}

export function MeetingHeading({ meeting, title }: { meeting: Meeting; title: string }) {
  return (
    <>
      <PadRow label={shortDate(meeting.startedAt)} labelClassName="@min-[620px]:pt-[11px]">
        <h1 className="font-serif text-[30px] leading-[1.15] font-semibold tracking-[-0.01em] text-ink text-balance">{title}</h1>
      </PadRow>
      <PadRow className="mt-2">
        <p className="text-[13px] text-graphite">{meetingSentence(meeting)}</p>
      </PadRow>
    </>
  );
}

/** Action items that check off at once and save in the background, reverting if the save fails. */
function useCheckedItems(id: string, note: MeetingNote): [ActionItem[], (index: number) => void] {
  const [items, setItems] = useState(note.actionItems);
  const [shownNote, setShownNote] = useState(note);
  if (shownNote !== note) {
    setShownNote(note);
    setItems(note.actionItems);
  }

  const toggle = (index: number) => {
    const next = items.map((item, i) => (i === index ? { ...item, done: !item.done } : item));
    setItems(next);
    void attempt(() => api.saveNote(id, { ...note, actionItems: next })).then((saved) => saved || setItems(items));
  };
  return [items, toggle];
}

function SectionHeading({ children }: { children: ReactNode }) {
  return <h2 className="font-serif text-[19px] leading-snug font-semibold text-ink">{children}</h2>;
}
