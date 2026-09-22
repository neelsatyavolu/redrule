import clsx from "clsx";
import { AlertTriangle, Search, Settings2 } from "lucide-react";
import { useMemo, type ButtonHTMLAttributes, type Ref } from "react";
import { api } from "../lib/api";
import { bytes, groupByDay, matchesSearch, sidebarSubtitle } from "../lib/format";
import { attempt, useStore, visibleMeetings, type Library } from "../lib/store";
import type { Meeting, SpeechModel } from "../lib/types";
import { useDialogs } from "./dialogs/dialogState";
import { MeetingContextMenu } from "./MeetingMenu";
import { IconButton, RecordingDot, Segmented, Spinner } from "./ui";

export function Sidebar() {
  const app = useStore((s) => s.app);
  const { library, search, selectedId, select, setLibrary, setSearch } = useStore();
  const openDialog = useDialogs((s) => s.open);

  const groups = useMemo(
    () => groupByDay(visibleMeetings(app, library).filter((m) => matchesSearch(m, search))),
    [app, library, search],
  );

  const changeLibrary = (next: Library) => {
    setLibrary(next);
    select(visibleMeetings(app, next)[0]?.id ?? null);
  };

  return (
    <aside className="flex h-full w-[260px] shrink-0 flex-col border-r border-rule bg-pad">
      {/* The traffic lights sit over this strip; it also drags the window. */}
      <div data-tauri-drag-region className="h-[52px] shrink-0" />

      <div className="space-y-2.5 px-3 pb-2">
        <RecordButton />
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
        <Segmented
          label="Library"
          value={library}
          onChange={changeLibrary}
          options={[
            { value: "meetings", label: "Meetings" },
            { value: "archive", label: "Archive" },
          ]}
        />
      </div>

      <nav aria-label="Meetings" className="min-h-0 flex-1 overflow-y-auto px-2 pb-3">
        {groups.length === 0 && <EmptyList library={library} searching={search.trim() !== ""} />}
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

function RecordButton() {
  const recording = useStore((s) => Boolean(s.app?.recordingId));
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
      onClick={() => void attempt(() => api.startRecording())}
      title="Record a meeting now (⇧⌘R)"
      className="flex h-9 w-full items-center justify-center gap-2 rounded-[8px] border border-rule bg-raised text-[13px] font-medium text-ink shadow-[0_1px_1px_rgb(0_0_0/0.04)] transition-colors hover:bg-paper"
    >
      <span className="size-2.5 rounded-full bg-margin" aria-hidden />
      Record
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

function EmptyList({ library, searching }: { library: Library; searching: boolean }) {
  const text = searching
    ? "No meeting titles match."
    : library === "archive"
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
