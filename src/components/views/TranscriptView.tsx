import { Popover } from "radix-ui";
import { useState } from "react";
import { api } from "../../lib/api";
import { distinctSpeakers, speakerKey, speakerLabel } from "../../lib/format";
import { attempt } from "../../lib/store";
import type { Meeting, TranscriptSegment } from "../../lib/types";
import { PadPage, PadRow } from "../Pad";
import { Button, TextInput } from "../ui";
import { MeetingHeading } from "./NoteView";
import { SpeakerSwatch, TranscriptRows, speakerColors } from "./TranscriptRows";

export function TranscriptView({ meeting, segments, title }: { meeting: Meeting; segments: TranscriptSegment[]; title: string }) {
  const colors = speakerColors(segments);
  return (
    <PadPage>
      <MeetingHeading meeting={meeting} title={title} />
      {segments.length === 0 ? (
        <PadRow className="mt-8">
          <p className="font-serif text-[15px] text-graphite italic">No transcript is available for this meeting.</p>
        </PadRow>
      ) : (
        <>
          <PadRow className="mt-6" label="Speakers">
            <div className="flex flex-wrap gap-1.5">
              {distinctSpeakers(segments).map((speaker) => (
                <SpeakerChip
                  key={speakerKey(speaker)}
                  meetingId={meeting.id}
                  speaker={speaker}
                  color={colors.get(speakerKey(speaker))}
                  readOnly={meeting.remote}
                />
              ))}
            </div>
            {!meeting.remote && (
              <p className="mt-2 text-[12px] text-graphite">
                Speaker labels are estimated from voices. Select one to name that person throughout this meeting.
              </p>
            )}
          </PadRow>
          <div className="mt-9">
            <TranscriptRows segments={segments} />
          </div>
        </>
      )}
    </PadPage>
  );
}

interface SpeakerChipProps {
  meetingId: string;
  speaker: TranscriptSegment;
  color?: string;
  readOnly?: boolean;
}

const CHIP = "inline-flex h-7 items-center gap-1.5 rounded-full border border-rule bg-raised px-3 text-[12.5px] font-medium text-ink";

function SpeakerChip({ meetingId, speaker, color, readOnly }: SpeakerChipProps) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState(speaker.speakerName ?? "");
  const label = speakerLabel(speaker);
  if (readOnly) {
    return (
      <span className={CHIP}>
        <SpeakerSwatch color={color} />
        {label}
      </span>
    );
  }

  const save = async () => {
    if (await attempt(() => api.renameSpeaker(meetingId, speakerKey(speaker), name))) setOpen(false);
  };

  return (
    <Popover.Root
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (next) setName(speaker.speakerName ?? "");
      }}
    >
      <Popover.Trigger asChild>
        <button
          type="button"
          title={`Rename ${label}`}
          className={`${CHIP} transition-colors hover:border-faint data-[state=open]:border-focus`}
        >
          <SpeakerSwatch color={color} />
          {label}
        </button>
      </Popover.Trigger>
      <Popover.Portal>
        <Popover.Content side="bottom" align="start" sideOffset={6} className="rise z-50 w-[280px] rounded-[12px] border border-rule bg-raised p-3.5 shadow-float">
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void save();
            }}
          >
            <label className="block text-[12px] font-medium text-graphite" htmlFor="speaker-name">
              Name for {label}
            </label>
            <TextInput
              id="speaker-name"
              autoFocus
              value={name}
              maxLength={100}
              placeholder={speaker.speakerID ? `Speaker ${speaker.speakerID}` : label}
              onChange={(event) => setName(event.target.value)}
              className="mt-1.5"
            />
            <p className="mt-2 text-[11.5px] leading-relaxed text-graphite">
              Leave blank to restore the original label. Rewrite the notes to use the new name there.
            </p>
            <div className="mt-3 flex justify-end gap-2">
              <Button size="sm" variant="ghost" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button size="sm" variant="primary" type="submit">
                Save name
              </Button>
            </div>
          </form>
        </Popover.Content>
      </Popover.Portal>
    </Popover.Root>
  );
}
