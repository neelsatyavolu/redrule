import AVFoundation
import MinutesCore

/// Captures both audio streams and turns them into transcript segments while the meeting runs.
final class RecordingPipeline {
    private struct Chunk {
        let speaker: Speaker
        let samples: [Float]
        let time: TimeInterval
    }

    private let transcriber: any Transcriber
    private let audioFolder: URL?
    private let mic = MicCapture()
    private let system = SystemAudioCapture()
    private var input: AsyncStream<Chunk>.Continuation?
    private var worker: Task<[TranscriptSegment], Never>?

    /// - Parameter audioFolder: where to keep `me.wav` and `them.wav`, or nil to keep no audio.
    init(transcriber: any Transcriber, audioFolder: URL?) {
        self.transcriber = transcriber
        self.audioFolder = audioFolder
    }

    func start(
        onSegment: @escaping @Sendable (TranscriptSegment) -> Void,
        onError: @escaping @Sendable (Error) -> Void
    ) async throws {
        let (stream, continuation) = AsyncStream.makeStream(of: Chunk.self)
        input = continuation
        let began = Date()
        let elapsed = { @Sendable in Date().timeIntervalSince(began) }

        do {
            try await system.start(
                onSamples: { continuation.yield(Chunk(speaker: .them, samples: $0, time: elapsed())) },
                onStopped: onError
            )
            try mic.start { continuation.yield(Chunk(speaker: .me, samples: $0, time: elapsed())) }
        } catch {
            await system.stop()
            continuation.finish()
            throw error
        }

        let transcriber = transcriber
        let writers = audioFolder.map { folder in
            [Speaker.me: WavWriter(url: folder.appendingPathComponent("me.wav")), .them: WavWriter(url: folder.appendingPathComponent("them.wav"))]
        } ?? [:]

        worker = Task.detached(priority: .userInitiated) {
            var windowers: [Speaker: AudioWindower] = [.me: AudioWindower(), .them: AudioWindower()]
            var reportedError = false
            var segments: [TranscriptSegment] = []

            func transcribe(_ window: AudioWindow, speaker: Speaker) async {
                do {
                    let text = try await transcriber.transcribe(window)
                    guard !text.isEmpty else { return }
                    let segment = TranscriptSegment(speaker: speaker, start: window.start, end: window.end, text: text)
                    segments.append(segment)
                    onSegment(segment)
                } catch {
                    if !reportedError { onError(error) }
                    reportedError = true
                }
            }

            for await chunk in stream {
                writers[chunk.speaker]?.write(chunk.samples)
                for window in windowers[chunk.speaker]!.append(chunk.samples, endingAt: chunk.time) {
                    await transcribe(window, speaker: chunk.speaker)
                }
            }
            for speaker in [Speaker.them, .me] {
                if let window = windowers[speaker]!.flush() { await transcribe(window, speaker: speaker) }
            }
            return segments
        }
    }

    /// Stops capture, transcribes the remaining audio, and returns every segment of the meeting.
    func stop() async -> [TranscriptSegment] {
        mic.stop()
        await system.stop()
        input?.finish()
        return await worker?.value ?? []
    }
}

/// Appends 16 kHz mono samples to a WAV file. Write failures are ignored: kept audio is a convenience.
private final class WavWriter: @unchecked Sendable {
    private let file: AVAudioFile?

    init(url: URL) {
        file = try? AVAudioFile(forWriting: url, settings: AudioFormat.transcription.settings)
    }

    func write(_ samples: [Float]) {
        guard let file, let buffer = AVAudioPCMBuffer(pcmFormat: AudioFormat.transcription, frameCapacity: AVAudioFrameCount(samples.count)) else { return }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        samples.withUnsafeBufferPointer { buffer.floatChannelData![0].update(from: $0.baseAddress!, count: samples.count) }
        try? file.write(from: buffer)
    }
}
