import { clock, distinctSpeakers, speakerKey, speakerLabel } from "../../lib/format";
import type { TranscriptSegment } from "../../lib/types";
import { PadRow } from "../Pad";

/** Muted voice colours, assigned in order of first appearance. "Me" is always ink. */
const VOICES = ["#2f7a5b", "#4a63a8", "#a0662c", "#8a4f8f", "#2a7d86", "#a24c4c", "#6b7a2e"];

export function speakerColors(segments: TranscriptSegment[]): Map<string, string> {
  const others = distinctSpeakers(segments).filter((s) => s.speaker !== "me");
  return new Map(others.map((segment, index) => [speakerKey(segment), VOICES[index % VOICES.length]]));
}

export function SpeakerSwatch({ color }: { color?: string }) {
  return (
    <span
      aria-hidden
      className="inline-block size-2 shrink-0 rounded-full"
      style={{ background: color ?? "var(--ink)" }}
    />
  );
}

/** Transcript lines on the pad, with the speaker and time in the margin. */
export function TranscriptRows({ segments }: { segments: TranscriptSegment[] }) {
  const colors = speakerColors(segments);
  return (
    <div className="selectable space-y-4">
      {segments.map((segment) => {
        const key = speakerKey(segment);
        return (
          <PadRow
            key={`${key}-${segment.start}`}
            label={
              <span className="inline-flex flex-col items-start @min-[620px]:items-end">
                <span className="inline-flex max-w-full items-center gap-1.5 font-medium text-ink">
                  <SpeakerSwatch color={colors.get(key)} />
                  <span className="truncate">{speakerLabel(segment)}</span>
                </span>
                <span className="text-faint">{clock(segment.start)}</span>
              </span>
            }
          >
            <p
              className="font-serif text-[15px] leading-[1.65]"
              style={{ color: segment.speaker === "me" ? "var(--ink)" : "color-mix(in srgb, var(--ink) 84%, var(--paper))" }}
            >
              {segment.text}
            </p>
          </PadRow>
        );
      })}
    </div>
  );
}
