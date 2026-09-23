// Dev-only: runs the UI in a plain browser against sample data, for design review.
// Open http://localhost:1420/preview.html?view=notes (notes | transcript | live | loading | writing | failed | empty | settings | transcription | onboarding | share | tags | folder | newFolder | move | search)
import { mockIPC } from "@tauri-apps/api/mocks";
import type { AppState, Meeting, MeetingDetail } from "../lib/types";

const view = new URLSearchParams(location.search).get("view") ?? "notes";
/** Notes for the first meeting are being written on this Mac. */
const writingNotes = view === "loading" || view === "writing";
if (new URLSearchParams(location.search).get("theme") === "dark") document.documentElement.style.colorScheme = "dark";

const at = (daysAgo: number, hour: number, minute = 0) => {
  const d = new Date();
  d.setDate(d.getDate() - daysAgo);
  d.setHours(hour, minute, 0, 0);
  return d.toISOString();
};
const plus = (iso: string, minutes: number) => new Date(Date.parse(iso) + minutes * 60_000).toISOString();

const FOLDER = "f".repeat(64);

const meetings: Meeting[] = [
  { id: "M1", title: "Q4 roadmap review", app: "zoom", startedAt: at(0, 14, 5), endedAt: plus(at(0, 14, 5), 42), status: writingNotes ? "summarizing" : "done", tags: ["Product", "Planning"], folderId: FOLDER },
  { id: "M2", title: "Pricing page copy", app: "googleMeet", startedAt: at(0, 10, 30), endedAt: plus(at(0, 10, 30), 18), status: view === "failed" ? "failed" : "done", errorMessage: "Connect ChatGPT or Grok in Settings to generate notes." },
  { id: "M3", title: "Hiring sync with Priya", app: "zoom", startedAt: at(1, 16), endedAt: plus(at(1, 16), 27), status: "done", tags: ["Hiring"] },
  { id: "M4", title: "Weekly design critique", app: "googleMeet", startedAt: at(1, 11), endedAt: plus(at(1, 11), 55), status: "done" },
  { id: "M5", title: "Vendor contract call", app: "manual", startedAt: at(3, 9, 15), endedAt: plus(at(3, 9, 15), 71), status: "done" },
  { id: "M6", title: "Onboarding interview 3", app: "zoom", startedAt: at(4, 15), endedAt: plus(at(4, 15), 33), status: "done", tags: ["Hiring"] },
];

const folderMeetings: Meeting[] = [
  { id: "R1", title: "Acme onboarding kickoff", app: "zoom", startedAt: at(0, 11, 15), endedAt: plus(at(0, 11, 15), 36), status: "done", folderId: FOLDER, remote: true, recordedBy: "Dana" },
  { id: "R2", title: "Acme security review", app: "googleMeet", startedAt: at(2, 15), endedAt: plus(at(2, 15), 48), status: "done", folderId: FOLDER, remote: true, recordedBy: "Priya" },
];

const live: Meeting = { id: "LIVE", title: "New meeting", app: "manual", startedAt: new Date(Date.now() - 754_000).toISOString(), status: "recording" };

const transcript = [
  { speaker: "me", start: 2, end: 14, text: "Thanks for making time. I want to leave with a decision on whether the export feature slips to January." },
  { speaker: "them", speakerID: "1", speakerName: "Dana", start: 15, end: 41, text: "From the platform side we can make December if we drop the CSV variant and ship PDF only. The API work is already done." },
  { speaker: "them", speakerID: "2", start: 42, end: 63, text: "Support would rather have CSV, honestly. Most of the tickets are people wanting to get data into spreadsheets." },
  { speaker: "me", start: 64, end: 80, text: "Okay. Then let's ship CSV first in December and follow with PDF in January. Dana, can you confirm the estimate by Friday?" },
  { speaker: "them", speakerID: "1", speakerName: "Dana", start: 81, end: 88, text: "Yes, I'll have it by Friday." },
] as MeetingDetail["transcript"];

const detail: MeetingDetail = {
  transcript,
  share: null,
  note: {
    title: "Q4 roadmap review",
    tldr: "Export ships in December as CSV only, with PDF following in January. Dana confirms the platform estimate by Friday; the pricing experiment waits until export is out.",
    sections: [
      { heading: "Export feature", bullets: ["Platform can hit December if scope is cut to one format; the API work is complete.", "Support tickets overwhelmingly ask for spreadsheet export, so CSV goes first.", "PDF export moves to the January release."] },
      { heading: "Pricing experiment", bullets: ["Paused until export ships to avoid two changes landing in the same billing cycle.", "Revisit the annual-plan discount in the first January planning session."] },
    ],
    decisions: ["Ship CSV export in December; PDF follows in January.", "Hold the pricing experiment until after the export launch."],
    actionItems: [
      { owner: "Dana", task: "Confirm the December estimate for CSV export by Friday." },
      { owner: "Me", task: "Tell support the CSV timeline and ask for the top three column requests." },
      { owner: null, task: "Book a January planning session for the pricing experiment." },
    ],
  },
};

const state: AppState = {
  meetings: view === "empty" ? [] : view === "live" ? [live, ...meetings] : meetings,
  recordingId: view === "live" ? "LIVE" : null,
  liveSegments: view === "live" ? transcript.slice(0, 3) : [],
  banner: null,
  speechModel: view === "live" ? { state: "ready" } : { state: "loading", progress: { downloadedBytes: 214_000_000, totalBytes: 487_000_000, stage: "Downloading speech model" } },
  noteModel: null,
  askModel: null,
  notesProgress: writingNotes ? { meetingId: "M1", stage: view === "loading" ? "loading" : "writing", percent: view === "loading" ? 63 : 42 } : null,
  connected: view === "failed" ? [] : ["codex"],
  connecting: null,
  connectionError: null,
  permissions: { microphone: view !== "onboarding", screenRecording: true },
  settings: {
    modelChoiceId: "codex:gpt-6-astra",
    askModelChoiceId: "",
    keepAudio: false,
    showInDock: true,
    microphoneId: "",
    onboarded: view !== "onboarding",
    speechModelId: "parakeet-v3",
    speakerModelId: "accurate",
    consentReminder: true,
    consentNotice: "Heads up: I'm recording this call to take notes. Let me know if you'd rather I didn't.",
    crashReports: false,
  },
  sharingBusy: false,
  revision: 1,
  storageError: null,
  folders: [{ id: FOLDER, name: "Acme team", owner: true, unavailable: false }],
  folderMeetings,
  displayName: "Neel",
};

mockIPC((command) => {
  switch (command) {
    case "get_state":
      return state;
    case "meeting_detail":
      return view === "failed" || writingNotes ? { ...detail, note: null } : detail;
    case "ask_meeting":
      return new Promise((resolve) =>
        setTimeout(
          () =>
            resolve(
              "CSV export ships in December and PDF follows in January [01:04].\n\n- Dana confirms the estimate by Friday.\n- The pricing experiment waits until export is out.",
            ),
          1200,
        ),
      );
    case "model_choices":
      return [
        { id: "codex:gpt-6-astra", provider: "codex", model: "gpt-6-astra", label: "GPT-6 Astra", effort: "low" },
        { id: "grok:grok-4.7", provider: "grok", model: "grok-4.7", label: "Grok 4.7", effort: "low" },
      ];
    case "local_models":
      return {
        speech: [
          { id: "parakeet-v3", name: "Parakeet v3", description: "Accurate in English and 24 other European languages, and detects the language by itself.", languages: "25 European languages", sizeMb: 464, installed: true, recommended: true },
          { id: "parakeet-v2", name: "Parakeet v2 English", description: "English only. Slightly ahead of v3 on English benchmarks, about the same in meetings. Same speed.", languages: "English", sizeMb: 460, installed: false, recommended: false },
          { id: "parakeet-lite", name: "Parakeet Lite English", description: "English only. A quarter of the processing and a fifth of the download, with a few more mistakes.", languages: "English", sizeMb: 103, installed: true, recommended: false },
        ],
        speaker: [
          { id: "standard", name: "Standard", description: "The smallest download. Fine for one-to-one calls; can merge similar voices in group calls.", languages: null, sizeMb: 32, installed: true, recommended: false },
          { id: "accurate", name: "Accurate", description: "Keeps similar voices apart in group calls, where Standard can merge them. Just as fast.", languages: null, sizeMb: 108, installed: false, recommended: true },
        ],
        notes: [
          { id: "qwen3.5-4b", name: "Qwen3.5 4B", description: "Writes notes in a minute or two and stays light on memory and battery.", languages: null, sizeMb: 2741, installed: false, recommended: true },
          { id: "qwen3.5-9b", name: "Qwen3.5 9B", description: "Sharper notes for long or technical meetings. About twice as slow and needs 16 GB of memory or more.", languages: null, sizeMb: 5681, installed: false, recommended: false },
        ],
        hardware: { memoryGb: 16, appleSilicon: true, cores: 10 },
      };
    case "search_meetings":
      return [{ id: "M2", snippet: "Them: Let's keep the free tier at three seats and revisit pricing in Q1." }];
    case "export_meeting":
      return true;
    case "microphones":
      return [{ id: "a", name: "MacBook Pro Microphone" }, { id: "b", name: "AirPods Pro" }];
    default:
      return null;
  }
}, { shouldMockEvents: true });

export const previewView = view;
