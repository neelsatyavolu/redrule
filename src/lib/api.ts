import { invoke } from "@tauri-apps/api/core";
import type {
  ApiProvider,
  AppState,
  Calendar,
  Exchange,
  MeetingApp,
  MeetingDetail,
  MeetingNote,
  LocalModels,
  Microphone,
  ModelKind,
  ModelOption,
  ProviderId,
  SearchHit,
  Settings,
  ExportFormat,
} from "./types";

/** Typed wrappers for the backend commands in src-tauri/src/commands.rs. */
export const api = {
  appReady: () => invoke<void>("app_ready"),
  getState: () => invoke<AppState>("get_state"),
  meetingDetail: (id: string) => invoke<MeetingDetail>("meeting_detail", { id }),
  startRecording: (source: MeetingApp = "manual", folder?: string) => invoke<void>("start_recording", { source, folder }),
  stopRecording: () => invoke<void>("stop_recording"),
  generateNotes: (id: string) => invoke<void>("generate_notes", { id }),
  askMeeting: (id: string, question: string, history: Exchange[]) =>
    invoke<string>("ask_meeting", { id, question, history }),
  searchMeetings: (query: string) => invoke<SearchHit[]>("search_meetings", { query }),
  renameMeeting: (id: string, title: string) => invoke<void>("rename_meeting", { id, title }),
  setArchived: (id: string, archived: boolean) => invoke<void>("set_archived", { id, archived }),
  setTags: (id: string, tags: string[]) => invoke<void>("set_tags", { id, tags }),
  setMeetingFolder: (id: string, folderId: string | null) => invoke<void>("set_meeting_folder", { id, folderId }),
  createFolder: (name: string, yourName: string) => invoke<string>("create_folder", { name, yourName }),
  joinFolder: (link: string, yourName: string) => invoke<string>("join_folder", { link, yourName }),
  renameFolder: (id: string, name: string) => invoke<void>("rename_folder", { id, name }),
  resetFolderLink: (id: string) => invoke<string>("reset_folder_link", { id }),
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  leaveFolder: (id: string) => invoke<void>("leave_folder", { id }),
  copyFolderLink: (id: string) => invoke<void>("copy_folder_link", { id }),
  deleteMeeting: (id: string) => invoke<void>("delete_meeting", { id }),
  saveNote: (id: string, note: MeetingNote) => invoke<void>("save_note", { id, note }),
  copyMarkdown: (id: string) => invoke<void>("copy_markdown", { id }),
  /** Resolves false when the save panel was cancelled. */
  exportMeeting: (id: string, format: ExportFormat) => invoke<boolean>("export_meeting", { id, format }),
  copyConsentNotice: () => invoke<void>("copy_consent_notice"),
  renameSpeaker: (id: string, key: string, name: string) => invoke<void>("rename_speaker", { id, key, name }),
  publishShare: (id: string, includeTranscript: boolean) => invoke<string>("publish_share", { id, includeTranscript }),
  revokeShare: (id: string) => invoke<void>("revoke_share", { id }),
  connect: (provider: ProviderId) => invoke<void>("connect", { provider }),
  submitPastedCode: (text: string) => invoke<void>("submit_pasted_code", { text }),
  cancelConnecting: () => invoke<void>("cancel_connecting"),
  disconnect: (provider: ProviderId) => invoke<void>("disconnect", { provider }),
  /** Checks the key with the provider, then saves it. `baseUrl` and `model` are for a custom server. */
  saveApiKey: (provider: ApiProvider, key: string, baseUrl = "", model = "") =>
    invoke<void>("save_api_key", { provider, key, baseUrl, model }),
  removeApiKey: (provider: ApiProvider) => invoke<void>("remove_api_key", { provider }),
  requestMicrophone: () => invoke<void>("request_microphone"),
  requestScreenRecording: () => invoke<void>("request_screen_recording"),
  requestCalendar: () => invoke<void>("request_calendar"),
  calendars: () => invoke<Calendar[]>("calendars"),
  updateSettings: (patch: Partial<Settings>) => invoke<Settings>("update_settings", { patch }),
  microphones: () => invoke<Microphone[]>("microphones"),
  modelChoices: () => invoke<ModelOption[]>("model_choices"),
  retrySpeechModel: () => invoke<void>("retry_speech_model"),
  retryNoteModel: () => invoke<void>("retry_note_model"),
  localModels: () => invoke<LocalModels>("local_models"),
  removeLocalModel: (kind: ModelKind, id: string) => invoke<void>("remove_local_model", { kind, id }),
  dismissBanner: () => invoke<void>("dismiss_banner"),
  showMainWindow: () => invoke<void>("show_main_window"),
  revealMeeting: (id: string) => invoke<void>("reveal_meeting", { id }),
  /** Checks now and installs a newer version. Resolves to the version waiting for a restart, if any. */
  checkForUpdates: () => invoke<string | null>("check_for_updates"),
  restartToUpdate: () => invoke<void>("restart_to_update"),
  reportError: (message: string, stack: string | null) => invoke<void>("report_error", { message, stack }),
};

/** Command errors arrive as their display string. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return "Something went wrong.";
}
