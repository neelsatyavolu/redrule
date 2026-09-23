import type { Meeting, MeetingApp, TranscriptSegment } from "./types";

export const APP_NAMES: Record<MeetingApp, string> = {
  zoom: "Zoom",
  googleMeet: "Google Meet",
  manual: "Recording",
};

/** "04:05" or "1:02:05". */
export function clock(seconds: number): string {
  const total = Math.max(0, Math.floor(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

/** "under a minute", "42 minutes", "1 hr 5 min". */
export function length(seconds: number): string {
  const minutes = Math.round(seconds / 60);
  if (minutes < 1) return "under a minute";
  if (minutes < 60) return minutes === 1 ? "1 minute" : `${minutes} minutes`;
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return m === 0 ? `${h} hr` : `${h} hr ${m} min`;
}

export function durationSeconds(meeting: Meeting): number | null {
  if (!meeting.endedAt) return null;
  return (Date.parse(meeting.endedAt) - Date.parse(meeting.startedAt)) / 1000;
}

const time = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const weekday = new Intl.DateTimeFormat(undefined, { weekday: "long" });
const dayMonth = new Intl.DateTimeFormat(undefined, { day: "numeric", month: "short" });
const longDay = new Intl.DateTimeFormat(undefined, { weekday: "long", day: "numeric", month: "long" });

export function timeOfDay(iso: string): string {
  return time.format(new Date(iso));
}

export function shortDate(iso: string): string {
  return dayMonth.format(new Date(iso));
}

/** "Monday at 2:05 PM, 42 minutes on Zoom". */
export function meetingSentence(meeting: Meeting): string {
  const start = new Date(meeting.startedAt);
  const duration = durationSeconds(meeting);
  const lengthPart = duration === null ? "" : `, ${length(duration)}`;
  const place = meeting.app === "manual" ? "" : ` on ${APP_NAMES[meeting.app]}`;
  return `${weekday.format(start)} at ${time.format(start)}${lengthPart}${place}`;
}

export function sidebarSubtitle(meeting: Meeting): string {
  const duration = durationSeconds(meeting);
  const start = timeOfDay(meeting.startedAt);
  return duration === null ? start : `${start}, ${length(duration)}`;
}

function startOfDay(date: Date): number {
  return new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime();
}

export interface DayGroup {
  key: number;
  label: string;
  meetings: Meeting[];
}

/** Groups meetings by calendar day, newest first, labelled "Today", "Yesterday" or the date. */
export function groupByDay(meetings: Meeting[], now: Date = new Date()): DayGroup[] {
  const today = startOfDay(now);
  const yesterday = startOfDay(new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1));
  const groups = new Map<number, Meeting[]>();
  for (const meeting of meetings) {
    const day = startOfDay(new Date(meeting.startedAt));
    groups.set(day, [...(groups.get(day) ?? []), meeting]);
  }
  return [...groups.entries()]
    .sort(([a], [b]) => b - a)
    .map(([key, items]) => ({
      key,
      label: key === today ? "Today" : key === yesterday ? "Yesterday" : longDay.format(new Date(key)),
      meetings: [...items].sort((a, b) => Date.parse(b.startedAt) - Date.parse(a.startedAt)),
    }));
}

export function speakerKey(segment: TranscriptSegment): string {
  return `${segment.speaker}:${segment.speakerID ?? "source"}`;
}

export function speakerLabel(segment: TranscriptSegment): string {
  if (segment.speakerName) return segment.speakerName;
  if (segment.speakerID) return `Speaker ${segment.speakerID}`;
  return segment.speaker === "me" ? "Me" : "Them";
}

/** One entry per distinct speaker, in order of first appearance. */
export function distinctSpeakers(segments: TranscriptSegment[]): TranscriptSegment[] {
  const seen = new Set<string>();
  return segments.filter((segment) => {
    const key = speakerKey(segment);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function bytes(count: number): string {
  if (count < 1024 * 1024) return `${Math.max(1, Math.round(count / 1024))} KB`;
  return `${Math.round(count / (1024 * 1024))} MB`;
}

export function sameTag(a: string, b: string): boolean {
  return a.localeCompare(b, undefined, { sensitivity: "base" }) === 0;
}

export function hasTag(meeting: Meeting, tag: string): boolean {
  return (meeting.tags ?? []).some((t) => sameTag(t, tag));
}

/** Every tag in use, once each (first spelling wins), in alphabetical order. */
export function allTags(meetings: Meeting[]): string[] {
  const tags: string[] = [];
  for (const tag of meetings.flatMap((m) => m.tags ?? [])) {
    if (!tags.some((t) => sameTag(t, tag))) tags.push(tag);
  }
  return tags.sort((a, b) => a.localeCompare(b, undefined, { sensitivity: "base" }));
}

/** Case-insensitive match on title, for the sidebar search. */
export function matchesSearch(meeting: Meeting, query: string): boolean {
  const needle = query.trim().toLowerCase();
  return needle === "" || meeting.title.toLowerCase().includes(needle);
}
