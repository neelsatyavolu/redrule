import AppKit
import CoreAudio
import Darwin
import MinutesCore

/// Polls the system for signs of a call and feeds `MeetingDetectorLogic`.
@MainActor
final class MeetingDetector {
    private static let pollInterval: Duration = .seconds(3)
    private nonisolated static let browserNames: Set<String> = [
        "Google Chrome", "Safari", "Arc", "Dia", "Microsoft Edge", "Brave Browser", "Firefox", "Vivaldi",
    ]

    private var logic = MeetingDetectorLogic()
    private var task: Task<Void, Never>?

    func start(onEvent: @escaping @MainActor (DetectionEvent) -> Void) {
        task?.cancel()
        task = Task { [weak self] in
            while !Task.isCancelled {
                let snapshot = await Task.detached(priority: .utility) { Self.snapshot() }.value
                guard let self else { return }
                let (next, event) = self.logic.ingest(snapshot)
                self.logic = next
                if let event { onEvent(event) }
                try? await Task.sleep(for: Self.pollInterval)
            }
        }
    }

    nonisolated static func snapshot() -> DetectionSnapshot {
        DetectionSnapshot(processNames: processNames(), micUserBundleIDs: micUserBundleIDs(), browserWindowTitles: browserWindowTitles())
    }

    // MARK: - Processes

    private nonisolated static func processNames() -> Set<String> {
        let count = proc_listallpids(nil, 0)
        guard count > 0 else { return [] }
        var pids = [pid_t](repeating: 0, count: Int(count) + 64)
        let filled = proc_listallpids(&pids, Int32(pids.count * MemoryLayout<pid_t>.size))
        guard filled > 0 else { return [] }
        return Set(pids.prefix(Int(filled)).compactMap { pid in
            var name = [CChar](repeating: 0, count: 256)
            return proc_name(pid, &name, UInt32(name.count)) > 0 ? String(cString: name) : nil
        })
    }

    // MARK: - Microphone users

    /// Bundle ids of other processes with a running audio input, from CoreAudio's process objects.
    private nonisolated static func micUserBundleIDs() -> Set<String> {
        let system = AudioObjectID(kAudioObjectSystemObject)
        var address = propertyAddress(kAudioHardwarePropertyProcessObjectList)
        var size: UInt32 = 0
        guard AudioObjectGetPropertyDataSize(system, &address, 0, nil, &size) == noErr, size > 0 else { return [] }
        var processes = [AudioObjectID](repeating: 0, count: Int(size) / MemoryLayout<AudioObjectID>.size)
        guard AudioObjectGetPropertyData(system, &address, 0, nil, &size, &processes) == noErr else { return [] }

        let ownPID = getpid()
        return Set(processes.compactMap { process in
            guard let running: UInt32 = value(of: kAudioProcessPropertyIsRunningInput, on: process), running != 0,
                  let pid: pid_t = value(of: kAudioProcessPropertyPID, on: process), pid != ownPID
            else { return nil }
            return bundleID(of: process)
        })
    }

    private nonisolated static func propertyAddress(_ selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
        AudioObjectPropertyAddress(mSelector: selector, mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
    }

    private nonisolated static func value<T>(of selector: AudioObjectPropertySelector, on object: AudioObjectID) -> T? {
        var address = propertyAddress(selector)
        var size = UInt32(MemoryLayout<T>.size)
        let pointer = UnsafeMutablePointer<T>.allocate(capacity: 1)
        defer { pointer.deallocate() }
        guard AudioObjectGetPropertyData(object, &address, 0, nil, &size, pointer) == noErr else { return nil }
        return pointer.pointee
    }

    private nonisolated static func bundleID(of process: AudioObjectID) -> String? {
        var address = propertyAddress(kAudioProcessPropertyBundleID)
        var size = UInt32(MemoryLayout<CFString?>.size)
        var result: Unmanaged<CFString>?
        guard AudioObjectGetPropertyData(process, &address, 0, nil, &size, &result) == noErr, let result else { return nil }
        let id = result.takeRetainedValue() as String
        return id.isEmpty ? nil : id
    }

    // MARK: - Windows

    /// Window titles are only readable once Screen Recording permission has been granted.
    private nonisolated static func browserWindowTitles() -> [String] {
        let info = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
        return info.compactMap { window in
            guard let owner = window[kCGWindowOwnerName as String] as? String, browserNames.contains(owner),
                  let title = window[kCGWindowName as String] as? String, !title.isEmpty
            else { return nil }
            return title
        }
    }
}
