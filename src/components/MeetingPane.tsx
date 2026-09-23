import { Copy, Ellipsis, RefreshCw, Share } from "lucide-react";
import { useState } from "react";
import { api } from "../lib/api";
import { attempt, useStore } from "../lib/store";
import { AskBar } from "./AskBar";
import type { Meeting } from "../lib/types";
import { useMeetingDetail } from "../lib/useMeetingDetail";
import { useDialogs } from "./dialogs/dialogState";
import { MeetingDropdown } from "./MeetingMenu";
import { IconButton, RecordingDot, Segmented, Spinner } from "./ui";
import { NoteView } from "./views/NoteView";
import { EmptyView, LiveView, ProcessingView } from "./views/StateViews";
import { TranscriptView } from "./views/TranscriptView";

type Tab = "notes" | "transcript";

export function MeetingPane() {
  const app = useStore((s) => s.app);
  const selectedId = useStore((s) => s.selectedId);
  const meeting = app?.meetings.find((m) => m.id === selectedId);

  if (!app) return <Chrome />;
  if (!meeting) return (
    <Chrome>
      <EmptyView />
    </Chrome>
  );
  if (meeting.id === app.recordingId) {
    return (
      <Chrome
        center={
          <span className="flex items-center gap-2 text-[12.5px] font-medium text-ink">
            <RecordingDot size={7} />
            Recording
          </span>
        }
      >
        <LiveView meeting={meeting} />
      </Chrome>
    );
  }
  // Keyed so switching meetings resets the tab and scroll position.
  return <SavedMeeting key={meeting.id} meeting={meeting} revision={app.revision} />;
}

function SavedMeeting({ meeting, revision }: { meeting: Meeting; revision: number }) {
  const [tab, setTab] = useState<Tab>("notes");
  const detail = useMeetingDetail(meeting.id, revision);
  const sharingBusy = useStore((s) => s.app?.sharingBusy ?? false);
  const openDialog = useDialogs((s) => s.open);

  if (detail.status !== "ready") {
    return (
      <Chrome>
        {detail.status === "error" ? (
          <p className="p-10 text-[13px] text-graphite">{detail.message}</p>
        ) : (
          <div className="grid h-full place-items-center">
            <Spinner />
          </div>
        )}
      </Chrome>
    );
  }

  const { note, transcript } = detail.detail;
  const finished = meeting.status === "done" && note !== null;
  // Someone else's meeting from a shared folder: read and copy only.
  const own = !meeting.remote;
  const actions = (
    <>
      {finished && (
        <>
          {own && (
            <IconButton label="Share" disabled={sharingBusy} onClick={() => openDialog({ kind: "share", meeting })}>
              <Share size={15} />
            </IconButton>
          )}
          <IconButton label="Copy as Markdown" onClick={() => void attempt(() => api.copyMarkdown(meeting.id), "Copied as Markdown")}>
            <Copy size={15} />
          </IconButton>
          {own && (
            <IconButton label="Rewrite notes from the transcript" onClick={() => void attempt(() => api.generateNotes(meeting.id))}>
              <RefreshCw size={15} />
            </IconButton>
          )}
        </>
      )}
      <MeetingDropdown meeting={meeting}>
        <IconButton label="More actions">
          <Ellipsis size={16} />
        </IconButton>
      </MeetingDropdown>
    </>
  );

  if (!finished) {
    return (
      <Chrome actions={actions}>
        <ProcessingView meeting={meeting} segments={transcript} />
      </Chrome>
    );
  }

  return (
    <Chrome
      actions={actions}
      center={
        <Segmented
          label="Meeting content"
          className="w-[210px]"
          value={tab}
          onChange={setTab}
          options={[
            { value: "notes", label: "Notes" },
            { value: "transcript", label: "Transcript" },
          ]}
        />
      }
    >
      <div key={tab} className="arrive h-full">
        {tab === "notes" ? (
          <NoteView meeting={meeting} note={note} />
        ) : (
          <TranscriptView meeting={meeting} segments={transcript} title={note.title} />
        )}
      </div>
      {own && <AskBar meetingId={meeting.id} />}
    </Chrome>
  );
}

interface ChromeProps {
  center?: React.ReactNode;
  actions?: React.ReactNode;
  children?: React.ReactNode;
}

/** The detail pane: a draggable toolbar strip over the page. */
function Chrome({ center, actions, children }: ChromeProps) {
  return (
    <main className="flex h-full min-w-0 flex-1 flex-col bg-paper">
      <header data-tauri-drag-region className="grid h-[52px] shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-3 border-b border-rule px-3">
        <div />
        <div className="flex justify-center">{center}</div>
        <div className="flex justify-end gap-0.5">{actions}</div>
      </header>
      <div className="relative min-h-0 flex-1">{children}</div>
    </main>
  );
}
