import clsx from "clsx";
import { AlertTriangle, Search, Settings2 } from "lucide-react";
import { useMemo, type ButtonHTMLAttributes, type Ref } from "react";
import { api } from "../lib/api";
import { allTags, bytes, groupByDay, matchesSearch, sidebarSubtitle } from "../lib/format";
import { attempt, keptTag, scopeFolder, useStore, visibleMeetings, type Library } from "../lib/store";
import type { FolderInfo, Meeting, SpeechModel } from "../lib/types";
import { useDialogs } from "./dialogs/dialogState";
import { MeetingContextMenu } from "./MeetingMenu";
import { FolderActions, ScopeMenu } from "./ScopeMenu";
import { IconButton, RecordingDot, Spinner } from "./ui";

const NO_FOLDERS: FolderInfo[] = [];

export function Sidebar() {
  const app = useStore((s) => s.app);
  const { library, search, selectedId, tag, select, setLibrary, setSearch, setTag } = useStore();
  const openDialog = useDialogs((s) => s.open);
  const folders = app?.folders ?? NO_FOLDERS;
  const folderId = scopeFolder(library);
  const folder = folders.find((f) => f.id === folderId);

  const tags = useMemo(() => allTags(visibleMeetings(app, library)), [app, library]);
  const groups = useMemo(
    () => groupByDay(visibleMeetings(app, library, tag).filter((m) => matchesSearch(m, search))),
    [app, library, tag, search],
  );

  const changeLibrary = (next: Library) => {
    const nextTag = keptTag(app, next, tag);
    setLibrary(next);
    setTag(nextTag);
    select(visibleMeetings(app, next, nextTag)[0]?.id ?? null);
  };

  const changeTag = (next: string | null) => {
    setTag(next);
    select(visibleMeetings(app, library, next)[0]?.id ?? null);
  };

  return (
    <aside className="flex h-full w-[260px] shrink-0 flex-col border-r border-rule bg-pad">
      {/* The traffic lights sit over this strip; it also drags the window. */}
      <div data-tauri-drag-region className="h-[52px] shrink-0" />

      <div className="space-y-2.5 px-3 pb-2">
        <RecordButton folder={folder} />
        <label className="flex h-7 items-center gap-2 rounded-[7px] bg-wash px-2 text-graphite focus-within:ring-2 focus-within:ring-focus">
          <Search size={13} aria-hidden />
          <input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search meetings"
            aria-label="Search meetings"
            className="h-full min-w-0 flex-1 bg-transparent text-[12.5px] text-ink placeholder:text-faint focus:outline-none"
          />
        </label>
        <div className="flex items-center gap-1">
          <ScopeMenu value={library} folders={folders} onChange={changeLibrary} />
          {folder && <FolderActions folder={folder} />}
        </div>
        {folder?.unavailable && (
          <p className="rounded-[7px] bg-margin/10 px-2 py-1.5 text-[11.5px] leading-snug text-margin">
            This folder's link no longer works. Its owner may have reset or deleted it.
          </p>
        )}
        {tags.length > 0 && <TagFilter tags={tags} value={tag} onChange={changeTag} />}
      </div>

      <nav aria-label="Meetings" className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
        {groups.length === 0 && <EmptyList folder={folderId !== null} archive={library === "archive"} searching={search.trim() !== ""} />}
        {groups.map((group) => (
          <section key={group.key} className="mt-3">
            <h2 className="px-2 pb-1 text-[11.5px] font-semibold text-graphite">{group.label}</h2>
            <ul>
              {group.meetings.map((meeting) => (
                <li key={meeting.id}>
                  <MeetingContextMenu meeting={meeting}>
                    <SidebarRow
                      meeting={meeting}
                      live={meeting.id === app?.recordingId}
                      selected={meeting.id === selectedId}
                      onSelect={() => select(meeting.id)}
                    />
                  </MeetingContextMenu>
                </li>
              ))}
            </ul>
          </section>
        ))}
      </nav>

      <footer className="flex items-center gap-1 border-t border-rule px-2 py-1.5">
        <SpeechModelStatus model={app?.speechModel} />
        <IconButton label="Settings" onClick={() => openDialog({ kind: "settings" })}>
          <Settings2 size={15} />
        </IconButton>
      </footer>
    </aside>
  );
}

function RecordButton({ folder }: { folder?: FolderInfo }) {
  const recording = useStore((s) => Boolean(s.app?.recordingId));
  const into = folder && !folder.unavailable ? folder : undefined;
  return recording ? (
    <button
      type="button"
      onClick={() => void attempt(api.stopRecording)}
      className="flex h-9 w-full items-center justify-center gap-2 rounded-[8px] bg-margin text-[13px] font-medium text-white transition-colors hover:bg-margin/90"
    >
      <span className="size-2.5 rounded-[2px] bg-white" aria-hidden />
      Stop and write notes
    </button>
  ) : (
    <button
      type="button"
      onClick={() => void attempt(() => api.startRecording("manual", into?.id))}
      title={into ? `Record a meeting into ${into.name}` : "Record a meeting now (⇧⌘R)"}
      className="flex h-9 w-full items-center justify-center gap-2 rounded-[8px] border border-rule bg-raised text-[13px] font-medium text-ink shadow-[0_1px_1px_rgb(0_0_0/0.04)] transition-colors hover:bg-paper"
    >
      <span className="size-2.5 shrink-0 rounded-full bg-margin" aria-hidden />
      <span className="truncate">{into ? `Record in ${into.name}` : "Record"}</span>
    </button>
  );
}

/** The context menu trigger passes its ref and handlers through `menuProps`. */
interface RowProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  meeting: Meeting;
  live: boolean;
  selected: boolean;
  onSelect: () => void;
  ref?: Ref<HTMLButtonElement>;
}

function SidebarRow({ meeting, live, selected, onSelect, ...menuProps }: RowProps) {
  return (
    <button
      type="button"
      onClick={onSelect}
      aria-current={selected ? "page" : undefined}
      {...menuProps}
      className={clsx(
        "flex w-full items-center gap-2 rounded-[8px] px-2 py-[7px] text-left transition-colors duration-75",
        selected ? "bg-raised shadow-[0_1px_2px_rgb(24_32_27/0.08)]" : "hover:bg-wash",
      )}
    >
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[13px] font-medium text-ink">{meeting.title}</span>
        <span className="block truncate text-[11.5px] text-graphite tabular">
          {live ? "Recording now" : sidebarSubtitle(meeting)}
          {meeting.recordedBy && <span> · {meeting.recordedBy}</span>}
          {meeting.tags?.length ? <span className="text-faint"> · {meeting.tags.join(", ")}</span> : null}
        </span>
      </span>
      <RowStatus meeting={meeting} live={live} />
    </button>
  );
}

function RowStatus({ meeting, live }: { meeting: Meeting; live: boolean }) {
  if (live) return <RecordingDot size={7} />;
  if (meeting.status === "failed")
    return <AlertTriangle size={13} className="shrink-0 text-graphite" aria-label="Notes could not be written" />;
  if (meeting.status !== "done") return <Spinner className="size-3" />;
  return null;
}

/** Tags in the current library; picking one shows only its meetings. Hidden until a meeting is tagged. */
function TagFilter({ tags, value, onChange }: { tags: string[]; value: string | null; onChange: (tag: string | null) => void }) {
  const options: { tag: string | null; label: string }[] = [{ tag: null, label: "All" }, ...tags.map((t) => ({ tag: t, label: t }))];
  return (
    <div role="radiogroup" aria-label="Filter by tag" className="flex max-h-[76px] flex-wrap gap-1 overflow-y-auto">
      {options.map((option) => {
        const checked = option.tag === value;
        return (
          <button
            key={option.tag ?? ""}
            type="button"
            role="radio"
            aria-checked={checked}
            onClick={() => onChange(checked ? null : option.tag)}
            className={clsx(
              "h-6 max-w-full truncate rounded-full px-2.5 text-[12px] font-medium transition-colors duration-100",
              checked ? "bg-ink text-paper" : "bg-wash text-graphite hover:text-ink",
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}

function EmptyList({ folder, archive, searching }: { folder: boolean; archive: boolean; searching: boolean }) {
  const text = searching
    ? "No meeting titles match."
    : folder
      ? "Meetings in this folder appear here. Record with the folder open, or move a meeting here from its menu."
      : archive
        ? "Archived meetings appear here."
        : "Recorded meetings appear here.";
  return <p className="px-2 pt-6 text-center text-[12px] text-faint">{text}</p>;
}

function SpeechModelStatus({ model }: { model?: SpeechModel }) {
  if (!model || model.state === "ready") {
    return <span className="flex-1 truncate px-1.5 text-[11.5px] text-faint">Transcribes on this Mac</span>;
  }
  if (model.state === "failed") {
    return (
      <button
        type="button"
        onClick={() => void attempt(api.retrySpeechModel)}
        title={model.message}
        className="flex-1 truncate px-1.5 text-left text-[11.5px] text-margin hover:underline"
      >
        Speech model failed. Retry
      </button>
    );
  }
  const progress = model.progress;
  const detail =
    progress && progress.totalBytes
      ? `${Math.round((progress.downloadedBytes / progress.totalBytes) * 100)}%`
      : progress && progress.downloadedBytes > 0
        ? bytes(progress.downloadedBytes)
        : "";
  return (
    <span
      title={progress?.stage ?? "Getting the speech model ready"}
      className="flex min-w-0 flex-1 items-center gap-2 px-1.5 text-[11.5px] text-graphite"
    >
      <Spinner className="size-3 shrink-0" />
      <span className="truncate">{detail ? `Speech model ${detail}` : "Preparing speech model"}</span>
    </span>
  );
}
