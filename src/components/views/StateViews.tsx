import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import { APP_NAMES, clock } from "../../lib/format";
import { attempt, useStore } from "../../lib/store";
import type { Meeting, SpeechModel, TranscriptSegment } from "../../lib/types";
import { useDialogs } from "../dialogs/dialogState";
import { PadPage, PadRow } from "../Pad";
import { Button, RecordingDot, Spinner } from "../ui";
import { MeetingHeading } from "./NoteView";
import { TranscriptRows } from "./TranscriptRows";

/** While a meeting is recorded: the running clock and the transcript as it arrives. */
export function LiveView({ meeting }: { meeting: Meeting }) {
  const segments = useStore((s) => s.app?.liveSegments ?? []);
  const speechModel = useStore((s) => s.app?.speechModel);
  const elapsed = useElapsed(meeting.startedAt);

  return (
    <PadPage>
      <PadRow label={<RecordingDot size={10} />} labelClassName="@min-[620px]:pt-[26px]">
        <div className="font-serif text-[64px] leading-none font-medium tracking-[-0.02em] text-ink tabular" aria-live="off">
          {clock(elapsed)}
        </div>
      </PadRow>
      <PadRow className="mt-3">
        <p className="text-[13px] text-graphite">
          {meeting.app === "manual"
            ? "Recording your microphone and this Mac's audio."
            : `Recording your ${APP_NAMES[meeting.app]} call.`}
        </p>
      </PadRow>
      <PadRow className="mt-5">
        <Button variant="record" onClick={() => void attempt(api.stopRecording)}>
          <span className="size-2 rounded-[2px] bg-white" aria-hidden />
          Stop and write notes
        </Button>
      </PadRow>
      <div className="mt-12">
        {segments.length === 0 ? (
          <PadRow>
            <p className="font-serif text-[15px] leading-relaxed text-graphite italic">{waitingMessage(speechModel)}</p>
          </PadRow>
        ) : (
          <TranscriptRows segments={segments} />
        )}
      </div>
    </PadPage>
  );
}

function useElapsed(startedAt: string): number {
  const start = Date.parse(startedAt);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  return (now - start) / 1000;
}

function waitingMessage(model?: SpeechModel): string {
  switch (model?.state) {
    case "loading":
      return "The speech model is still downloading. Audio is kept and transcribed as soon as it is ready.";
    case "failed":
      return "The speech model could not be loaded. Check the connection, then retry from the sidebar.";
    default:
      return "The transcript appears here a few seconds after people start talking.";
  }
}

/** After recording, while the transcript is finished and notes are written, or when that failed. */
export function ProcessingView({ meeting, segments }: { meeting: Meeting; segments: TranscriptSegment[] }) {
  const connected = useStore((s) => s.app?.connected ?? []);
  const openDialog = useDialogs((s) => s.open);

  return (
    <PadPage>
      <MeetingHeading meeting={meeting} title={meeting.title} />
      <PadRow className="mt-8">
        {meeting.status === "failed" ? (
          <div className="rounded-[10px] border border-rule bg-raised px-4 py-3.5">
            <p className="font-serif text-[15.5px] leading-relaxed text-ink">
              {meeting.errorMessage ?? "Notes could not be written."}
            </p>
            <div className="mt-3.5 flex flex-wrap gap-2">
              {segments.length > 0 && (
                <Button variant="primary" onClick={() => void attempt(() => api.generateNotes(meeting.id))}>
                  Write notes
                </Button>
              )}
              {connected.length === 0 && (
                <Button onClick={() => openDialog({ kind: "settings", tab: "accounts" })}>Connect an account</Button>
              )}
            </div>
          </div>
        ) : (
          <p className="flex items-center gap-2.5 font-serif text-[16px] text-graphite">
            <Spinner />
            {meeting.status === "transcribing" ? "Finishing the transcript" : "Writing notes"}
          </p>
        )}
      </PadRow>
      {segments.length > 0 && (
        <div className="mt-12">
          <TranscriptRows segments={segments} />
        </div>
      )}
    </PadPage>
  );
}

export function EmptyView() {
  const app = useStore((s) => s.app);
  const openDialog = useDialogs((s) => s.open);
  const needsSetup = app && (!app.permissions.microphone || !app.permissions.screenRecording || app.connected.length === 0);

  return (
    <PadPage className="pt-[14vh]">
      <PadRow>
        <h1 className="font-serif text-[30px] leading-tight font-semibold text-ink">No meetings yet</h1>
      </PadRow>
      <PadRow className="mt-4">
        <p className="max-w-[54ch] font-serif text-[16px] leading-[1.65] text-ink">
          Join a Zoom or Google Meet call and Redrule offers to take notes. It listens to the call and your microphone,
          transcribes on this Mac, and writes up the meeting when it ends.
        </p>
      </PadRow>
      <PadRow className="mt-6">
        <div className="flex flex-wrap gap-2">
          <Button variant="primary" onClick={() => void attempt(() => api.startRecording())}>
            Record now
          </Button>
          {needsSetup && <Button onClick={() => openDialog({ kind: "settings", tab: "permissions" })}>Finish setup</Button>}
        </div>
      </PadRow>
    </PadPage>
  );
}
