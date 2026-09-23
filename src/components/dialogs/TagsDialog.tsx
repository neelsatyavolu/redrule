import { X } from "lucide-react";
import { useMemo, useState, type KeyboardEvent } from "react";
import { api } from "../../lib/api";
import { allTags, sameTag } from "../../lib/format";
import { attempt, useStore } from "../../lib/store";
import type { Meeting } from "../../lib/types";
import { Button, Dialog, TextInput } from "../ui";

/** Matches the backend's limits in src-tauri/src/app/library.rs. */
const MAX_TAG = 40;
const MAX_TAGS = 20;

const includes = (list: string[], tag: string) => list.some((t) => sameTag(t, tag));

const CHIP = "inline-flex h-6 max-w-full items-center gap-1 rounded-full px-2.5 text-[12px] font-medium";

export function TagsDialog({ meeting, onClose }: { meeting: Meeting; onClose: () => void }) {
  const meetings = useStore((s) => s.app?.meetings);
  const known = useMemo(() => allTags(meetings ?? []), [meetings]);
  const [tags, setTags] = useState(meeting.tags ?? []);
  const [draft, setDraft] = useState("");
  const suggestions = known.filter((tag) => !includes(tags, tag));
  const full = tags.length >= MAX_TAGS;

  /** Adds a tag, reusing the spelling of one already in the library. */
  const withTag = (list: string[], raw: string): string[] => {
    const tag = raw.trim().replace(/^#+/, "").replace(/\s+/g, " ");
    if (tag === "" || list.length >= MAX_TAGS || includes(list, tag)) return list;
    return [...list, known.find((t) => sameTag(t, tag)) ?? tag];
  };
  const add = (raw: string) => {
    setTags(withTag(tags, raw));
    setDraft("");
  };
  const remove = (tag: string) => setTags(tags.filter((t) => t !== tag));

  // A tag typed but not yet added is kept too.
  const save = async () => {
    if (await attempt(() => api.setTags(meeting.id, withTag(tags, draft)))) onClose();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === ",") {
      event.preventDefault();
      add(draft);
    } else if (event.key === "Backspace" && draft === "" && tags.length > 0) {
      remove(tags[tags.length - 1]);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title="Tags"
      width={440}
      description="Tags group related meetings. Pick a tag in the sidebar to see only its meetings."
      footer={
        <>
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={() => void save()}>
            Save
          </Button>
        </>
      }
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (draft.trim() === "") void save();
          else add(draft);
        }}
        className="space-y-3"
      >
        {tags.length > 0 && (
          <ul aria-label="Tags on this meeting" className="flex flex-wrap gap-1.5">
            {tags.map((tag) => (
              <li key={tag} className={`${CHIP} bg-wash pr-1 text-ink`}>
                <span className="truncate">{tag}</span>
                <button
                  type="button"
                  aria-label={`Remove ${tag}`}
                  onClick={() => remove(tag)}
                  className="grid size-4 place-items-center rounded-full text-graphite hover:bg-rule hover:text-ink"
                >
                  <X size={11} />
                </button>
              </li>
            ))}
          </ul>
        )}
        <TextInput
          data-autofocus
          aria-label="Add a tag"
          placeholder={full ? `A meeting can have up to ${MAX_TAGS} tags` : "Add a tag, then press Return"}
          disabled={full}
          value={draft}
          maxLength={MAX_TAG}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={onKeyDown}
        />
        {suggestions.length > 0 && !full && (
          <div>
            <p className="pb-1.5 text-[11.5px] font-semibold text-graphite">Your tags</p>
            <div className="flex flex-wrap gap-1.5">
              {suggestions.map((tag) => (
                <button key={tag} type="button" onClick={() => add(tag)} className={`${CHIP} border border-rule text-graphite hover:bg-wash hover:text-ink`}>
                  <span className="truncate">{tag}</span>
                </button>
              ))}
            </div>
          </div>
        )}
      </form>
    </Dialog>
  );
}
