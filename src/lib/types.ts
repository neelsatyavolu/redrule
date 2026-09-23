// Mirrors the Rust types serialised by the backend (see src-tauri/src/app/state.rs).

export type Speaker = "me" | "them";
export type MeetingApp = "zoom" | "googleMeet" | "teams" | "slack" | "webex" | "faceTime" | "manual";
export type MeetingStatus = "recording" | "transcribing" | "summarizing" | "done" | "failed";
/** Accounts signed in through the browser. */
export type ProviderId = "codex" | "grok";
/** Providers reached with the person's own API key; "compatible" is any OpenAI-compatible server. */
export type ApiProvider = "openai" | "anthropic" | "gemini" | "compatible";
export type NoteProvider = ProviderId | ApiProvider;

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
  tags?: string[];
  /** The shared folder the meeting is in. */
  folderId?: string;
  /** Names from the calendar event, without the person recording. */
  attendees?: string[];
  /** Set on other people's meetings from a shared folder, which are read-only. */
  remote?: boolean;
  recordedBy?: string;
}

export interface FolderInfo {
  id: string;
  name: string;
  /** This Mac created the folder, so it can rename, reset or delete it. */
  owner: boolean;
  /** The link stopped working: the owner reset or deleted the folder. */
  unavailable: boolean;
}

export interface ActionItem {
  owner?: string | null;
  task: string;
  done?: boolean;
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
  /** Optional: only names meetings, never needed to record. */
  calendar: boolean;
}

export interface Settings {
  modelChoiceId: string;
  /** The model that answers questions about a meeting; empty to use the notes model. */
  askModelChoiceId: string;
  keepAudio: boolean;
  showInDock: boolean;
  microphoneId: string;
  onboarded: boolean;
  speechModelId: string;
  speakerModelId: string;
  /** Reminds the person to tell everyone on the call that it is being recorded. */
  consentReminder: boolean;
  /** What "Copy notice" puts on the clipboard. */
  consentNotice: string;
  /** Sends crash reports, when the build has somewhere to send them. Off by default. */
  crashReports: boolean;
  /** Takes titles and attendees from the calendar, once access is allowed. */
  useCalendar: boolean;
  /** Identifiers of calendars whose events never name a recording. */
  ignoredCalendars: string[];
  /** The OpenAI-compatible server's base URL and model name; empty when none is set up. */
  compatibleUrl: string;
  compatibleModel: string;
}

/** A calendar that holds events, as listed in settings. */
export interface Calendar {
  id: string;
  title: string;
  /** The account it syncs with, such as iCloud or a work address. */
  account: string;
}

/** A meeting whose title, tags, notes or transcript hold every word searched for. */
export interface SearchHit {
  id: string;
  /** The passage that matched; null when only the title did. */
  snippet: string | null;
}

export type ExportFormat = "markdown" | "text";

export type ModelKind = "speech" | "speaker" | "notes";

/** An on-device speech, speaker or note model from src-tauri/crates/engine/src/catalog.rs. */
export interface LocalModel {
  id: string;
  name: string;
  description: string;
  languages: string | null;
  sizeMb: number;
  installed: boolean;
  recommended: boolean;
}

export interface LocalModels {
  speech: LocalModel[];
  speaker: LocalModel[];
  notes: LocalModel[];
  hardware: { memoryGb: number; appleSilicon: boolean; cores: number };
}

/** Notes being written on this Mac: loading the model, then writing (an estimate). */
export interface NotesProgress {
  meetingId: string;
  stage: "loading" | "writing";
  percent: number;
}

export interface AppState {
  meetings: Meeting[];
  recordingId: string | null;
  liveSegments: TranscriptSegment[];
  banner: Banner | null;
  speechModel: SpeechModel;
  /** The on-device note model's download; null while notes are written with an account. */
  noteModel: SpeechModel | null;
  /** The on-device model that answers questions; null while an account answers them. */
  askModel: SpeechModel | null;
  notesProgress: NotesProgress | null;
  /** Signed-in accounts, then API providers with a saved key (or a custom server that is set up). */
  connected: NoteProvider[];
  connecting: ProviderId | null;
  connectionError: string | null;
  permissions: Permissions;
  settings: Settings;
  sharingBusy: boolean;
  revision: number;
  storageError: string | null;
  folders: FolderInfo[];
  /** Other people's meetings in shared folders. The store merges them into `meetings`. */
  folderMeetings: Meeting[];
  /** Shown on meetings this Mac adds to shared folders. */
  displayName: string;
  /** The build has somewhere to send crash reports; the setting is hidden otherwise. */
  crashReportsAvailable: boolean;
}

/** Model choices that write notes on this Mac, as stored in `Settings.modelChoiceId`. */
export const LOCAL_NOTES = "local:";

export function writesNotesLocally(settings: Settings): boolean {
  return settings.modelChoiceId.startsWith(LOCAL_NOTES);
}

/** Notes can be written: an account or API key is set up, or an on-device model is chosen. */
export function canWriteNotes(app: AppState): boolean {
  return app.connected.length > 0 || writesNotesLocally(app.settings);
}

/** An earlier question about a meeting and its answer. */
export interface Exchange {
  question: string;
  answer: string;
}

export interface Microphone {
  id: string;
  name: string;
}

export interface ModelOption {
  id: string;
  provider: NoteProvider;
  model: string;
  label: string;
  effort: string;
}
