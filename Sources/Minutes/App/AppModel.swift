import AppKit
import MinutesCore
import Observation

enum SpeechModelState: Equatable {
    case loading, ready
    case failed(String)
}

enum MeetingBanner: Equatable {
    case detected(MeetingApp)
    case ended
}

/// App-wide state. Recording lives in `AppModel+Recording`, accounts in `AppModel+Connections`.
@MainActor
@Observable
final class AppModel {
    private enum Key {
        static let modelChoice = "modelChoice"
        static let keepAudio = "keepAudio"
        static let onboarded = "onboarded"
    }

    // Library
    var meetings: [Meeting] = []
    var selection: UUID? { didSet { loadSelection() } }
    var selectedTranscript: [TranscriptSegment] = []
    var selectedNote: MeetingNote?
    var errorMessage: String?

    // Recording
    var recording: Meeting?
    var liveSegments: [TranscriptSegment] = []
    var banner: MeetingBanner? { didSet { onBannerChange?(banner) } }
    var speechModel: SpeechModelState = .loading
    @ObservationIgnored var onBannerChange: ((MeetingBanner?) -> Void)?
    @ObservationIgnored var pipeline: RecordingPipeline?
    @ObservationIgnored var startTask: Task<Bool, Never>?
    @ObservationIgnored var rawSegments: [TranscriptSegment] = []

    // Accounts
    var connected: Set<ProviderID> = []
    var connecting: ProviderID?
    var connectionError: String?
    @ObservationIgnored var connectTask: Task<Void, Never>?

    // Permissions
    var permissions = Permissions.current()

    // Settings
    var modelChoiceID: String { didSet { UserDefaults.standard.set(modelChoiceID, forKey: Key.modelChoice) } }
    var keepAudio: Bool { didSet { UserDefaults.standard.set(keepAudio, forKey: Key.keepAudio) } }
    var hasOnboarded: Bool { didSet { UserDefaults.standard.set(hasOnboarded, forKey: Key.onboarded) } }

    @ObservationIgnored let store: MeetingStore?
    @ObservationIgnored let oauth = OAuthService()
    @ObservationIgnored let transcriber: any Transcriber = ParakeetTranscriber()
    @ObservationIgnored private let detector = MeetingDetector()

    init() {
        let defaults = UserDefaults.standard
        modelChoiceID = ModelChoice.resolve(defaults.string(forKey: Key.modelChoice)).id
        keepAudio = defaults.bool(forKey: Key.keepAudio)
        hasOnboarded = defaults.bool(forKey: Key.onboarded)

        do {
            let store = try MeetingStore(root: MeetingStore.defaultRoot())
            try store.recoverInterrupted()
            self.store = store
        } catch {
            store = nil
            errorMessage = "Minutes cannot open its storage folder. \(error.localizedDescription)"
        }
        reloadMeetings()
        selection = meetings.first?.id
        loadSelection() // property observers do not run inside init
    }

    /// Work that should begin once the app has launched.
    func start() {
        detector.start { [weak self] event in self?.handle(event) }
        Task { await refreshConnections() }
        prepareSpeechModel()
    }

    func prepareSpeechModel() {
        speechModel = .loading
        Task {
            do {
                try await transcriber.prepare()
                speechModel = .ready
            } catch {
                speechModel = .failed(error.localizedDescription)
            }
        }
    }

    func refreshPermissions() {
        permissions = Permissions.current()
    }

    // MARK: - Library

    var selectedMeeting: Meeting? {
        meetings.first { $0.id == selection }
    }

    func reloadMeetings() {
        guard let store else { return }
        do {
            meetings = try store.list()
        } catch {
            errorMessage = "Meetings could not be loaded. \(error.localizedDescription)"
        }
    }

    func delete(_ meeting: Meeting) {
        guard meeting.id != recording?.id else { return }
        perform("The meeting could not be deleted.") { try $0.delete(meeting.id) }
        reloadMeetings()
        if selection == meeting.id { selection = meetings.first?.id }
    }

    func copyMarkdown() {
        guard let selectedNote else { return }
        NSPasteboard.general.clearContents()
        NSPasteboard.general.setString(selectedNote.markdown, forType: .string)
    }

    /// Saves a meeting and refreshes the list. Returns the saved meeting for chaining.
    @discardableResult
    func persist(_ meeting: Meeting) -> Meeting {
        perform("The meeting could not be saved.") { try $0.save(meeting) }
        reloadMeetings()
        return meeting
    }

    func perform(_ failure: String, _ work: (MeetingStore) throws -> Void) {
        guard let store else { return }
        do {
            try work(store)
        } catch {
            errorMessage = "\(failure) \(error.localizedDescription)"
        }
    }

    func loadSelection() {
        guard let store, let selection else {
            selectedTranscript = []
            selectedNote = nil
            return
        }
        selectedTranscript = (try? store.transcript(for: selection)).map(TranscriptMerger.merge) ?? []
        selectedNote = try? store.note(for: selection)
    }

    // MARK: - Detection

    private func handle(_ event: DetectionEvent) {
        switch event {
        case .detected(let app):
            if recording == nil { banner = .detected(app) }
        case .ended:
            banner = recording == nil ? nil : .ended
        }
    }
}
