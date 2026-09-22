// Mirrors the Rust types serialised by the backend (see src-tauri/src/app/state.rs).

export type Speaker = "me" | "them";
export type MeetingApp = "zoom" | "googleMeet" | "manual";
export type MeetingStatus = "recording" | "transcribing" | "summarizing" | "done" | "failed";
export type ProviderId = "codex" | "grok";

export interface TranscriptSegment {
  speaker: Speaker;
  start: number;
  end: number;
  text: string;
  speakerID?: string;
  speakerName?: string;
}

export interface Meeting {
  id: string;
  title: string;
  app: MeetingApp;
  startedAt: string;
  endedAt?: string;
  status: MeetingStatus;
  errorMessage?: string;
  archivedAt?: string;
}

export interface ActionItem {
  owner?: string | null;
  task: string;
}

export interface NoteSection {
  heading: string;
  bullets: string[];
}

export interface MeetingNote {
  title: string;
  tldr: string;
  sections: NoteSection[];
  decisions: string[];
  actionItems: ActionItem[];
}

export interface MeetingShare {
  id: string;
  includesTranscript: boolean;
  url: string;
}

export interface MeetingDetail {
  transcript: TranscriptSegment[];
  note: MeetingNote | null;
  share: MeetingShare | null;
}

export type Banner = { kind: "detected"; app: MeetingApp } | { kind: "ended" };

export interface ModelProgress {
  downloadedBytes: number;
  totalBytes?: number | null;
  stage: string;
}

export type SpeechModel =
  | { state: "loading"; progress: ModelProgress | null }
  | { state: "ready" }
  | { state: "failed"; message: string };

export interface Permissions {
  microphone: boolean;
  screenRecording: boolean;
}

export interface Settings {
  modelChoiceId: string;
  keepAudio: boolean;
  microphoneId: string;
  onboarded: boolean;
}

export interface AppState {
  meetings: Meeting[];
  recordingId: string | null;
  liveSegments: TranscriptSegment[];
  banner: Banner | null;
  speechModel: SpeechModel;
  connected: ProviderId[];
  connecting: ProviderId | null;
  connectionError: string | null;
  permissions: Permissions;
  settings: Settings;
  sharingBusy: boolean;
  revision: number;
  storageError: string | null;
}

export interface Microphone {
  id: string;
  name: string;
}

export interface ModelOption {
  id: string;
  provider: ProviderId;
  model: string;
  label: string;
  effort: string;
}
