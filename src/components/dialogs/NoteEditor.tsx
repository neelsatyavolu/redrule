import { Minus, Plus } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { api, errorMessage } from "../../lib/api";
import { attempt } from "../../lib/store";
import type { Meeting, MeetingNote } from "../../lib/types";
import { Button, Dialog, IconButton, TextArea, TextInput } from "../ui";

interface SectionDraft {
  key: number;
  heading: string;
  bullets: string;
}

interface ActionDraft {
  key: number;
  owner: string;
  task: string;
  done: boolean;
}

interface Draft {
  title: string;
  summary: string;
  sections: SectionDraft[];
  decisions: string;
  actions: ActionDraft[];
}

let nextKey = 0;
const key = () => ++nextKey;
const lines = (text: string) =>
  text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean);

function toDraft(note: MeetingNote): Draft {
  return {
    title: note.title,
    summary: note.tldr,
    sections: note.sections.map((s) => ({ key: key(), heading: s.heading, bullets: s.bullets.join("\n") })),
    decisions: note.decisions.join("\n"),
    actions: note.actionItems.map((a) => ({ key: key(), owner: a.owner ?? "", task: a.task, done: a.done ?? false })),
  };
}

function toNote(draft: Draft): MeetingNote {
  return {
    title: draft.title.trim(),
    tldr: draft.summary.trim(),
    sections: draft.sections.map((s) => ({ heading: s.heading.trim(), bullets: lines(s.bullets) })),
    decisions: lines(draft.decisions),
    actionItems: draft.actions
      .filter((a) => a.task.trim() !== "")
      .map((a) => ({ owner: a.owner.trim() || null, task: a.task.trim(), done: a.done })),
  };
}

export function NoteEditor({ meeting, onClose }: { meeting: Meeting; onClose: () => void }) {
  const [draft, setDraft] = useState<Draft | null>(null);
  const [failure, setFailure] = useState<string | null>(null);

  useEffect(() => {
    api
      .meetingDetail(meeting.id)
      .then(({ note }) => (note ? setDraft(toDraft(note)) : setFailure("No saved notes are available for this meeting.")))
      .catch((error) => setFailure(`The notes could not be opened. ${errorMessage(error)}`));
  }, [meeting.id]);

  const update = (patch: Partial<Draft>) => setDraft((current) => (current ? { ...current, ...patch } : current));
  const save = async () => {
    if (draft && (await attempt(() => api.saveNote(meeting.id, toNote(draft)), "Notes saved"))) onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title="Edit notes"
      width={660}
      description="Edits are saved on this Mac. Update the shared link to publish them."
      footer={
        <>
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" disabled={!draft || draft.title.trim() === ""} onClick={() => void save()}>
            Save notes
          </Button>
        </>
      }
    >
      {failure && <p className="text-[13px] text-margin">{failure}</p>}
      {draft && (
        <div className="space-y-6">
          <Field label="Title">
            <TextInput value={draft.title} maxLength={300} onChange={(e) => update({ title: e.target.value })} />
          </Field>
          <Field label="Summary">
            <TextArea value={draft.summary} onChange={(e) => update({ summary: e.target.value })} />
          </Field>

          <Field label="Sections" hint="One bullet per line.">
            <div className="space-y-3">
              {draft.sections.map((section) => (
                <div key={section.key} className="rounded-[10px] border border-rule bg-raised p-3">
                  <div className="flex gap-2">
                    <TextInput
                      aria-label="Heading"
                      placeholder="Heading"
                      value={section.heading}
                      className="font-medium"
                      onChange={(e) =>
                        update({ sections: draft.sections.map((s) => (s.key === section.key ? { ...s, heading: e.target.value } : s)) })
                      }
                    />
                    <IconButton
                      label="Remove section"
                      onClick={() => update({ sections: draft.sections.filter((s) => s.key !== section.key) })}
                    >
                      <Minus size={15} />
                    </IconButton>
                  </div>
                  <TextArea
                    aria-label="Bullets"
                    placeholder="Bullets, one per line"
                    value={section.bullets}
                    className="mt-2"
                    onChange={(e) =>
                      update({ sections: draft.sections.map((s) => (s.key === section.key ? { ...s, bullets: e.target.value } : s)) })
                    }
                  />
                </div>
              ))}
              <Button size="sm" variant="ghost" onClick={() => update({ sections: [...draft.sections, { key: key(), heading: "", bullets: "" }] })}>
                <Plus size={14} /> Add section
              </Button>
            </div>
          </Field>

          <Field label="Decisions" hint="One per line.">
            <TextArea value={draft.decisions} onChange={(e) => update({ decisions: e.target.value })} />
          </Field>

          <Field label="Action items">
            <div className="space-y-2">
              {draft.actions.map((action) => (
                <div key={action.key} className="flex items-start gap-2">
                  <TextInput
                    aria-label="Owner"
                    placeholder="Owner"
                    value={action.owner}
                    className="w-[140px] shrink-0"
                    onChange={(e) =>
                      update({ actions: draft.actions.map((a) => (a.key === action.key ? { ...a, owner: e.target.value } : a)) })
                    }
                  />
                  <TextArea
                    aria-label="Task"
                    placeholder="Task"
                    value={action.task}
                    className="min-h-8"
                    rows={1}
                    onChange={(e) =>
                      update({ actions: draft.actions.map((a) => (a.key === action.key ? { ...a, task: e.target.value } : a)) })
                    }
                  />
                  <IconButton
                    label="Remove action item"
                    onClick={() => update({ actions: draft.actions.filter((a) => a.key !== action.key) })}
                  >
                    <Minus size={15} />
                  </IconButton>
                </div>
              ))}
              <Button size="sm" variant="ghost" onClick={() => update({ actions: [...draft.actions, { key: key(), owner: "", task: "", done: false }] })}>
                <Plus size={14} /> Add action item
              </Button>
            </div>
          </Field>
        </div>
      )}
    </Dialog>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: ReactNode }) {
  return (
    <div>
      <div className="mb-1.5 flex items-baseline gap-2">
        <span className="text-[12.5px] font-semibold text-ink">{label}</span>
        {hint && <span className="text-[11.5px] text-faint">{hint}</span>}
      </div>
      {children}
    </div>
  );
}
