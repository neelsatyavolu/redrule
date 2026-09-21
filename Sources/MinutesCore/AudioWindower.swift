import Foundation

public struct AudioWindow: Equatable, Sendable {
    /// Seconds since the recording started.
    public let start: TimeInterval
    public let samples: [Float]

    public var end: TimeInterval { start + Double(samples.count) / AudioWindower.sampleRate }
}

/// Cuts a continuous 16 kHz stream into windows for transcription, preferring to cut in a pause.
/// Mutating by design: it sits on the audio path and must not copy its buffer on every callback.
public struct AudioWindower: Sendable {
    public static let sampleRate: Double = 16_000
    static let minWindow = 12.0
    static let maxWindow = 18.0
    static let minTail = 0.5
    static let frame = 1_600 // 100 ms
    static let silenceRMS: Float = 0.003

    private var buffer: [Float] = []
    private var bufferStart: TimeInterval = 0

    public init() {}

    /// Adds samples whose last sample was captured `endingAt` seconds into the recording.
    public mutating func append(_ samples: [Float], endingAt time: TimeInterval) -> [AudioWindow] {
        if buffer.isEmpty {
            bufferStart = max(0, time - Double(samples.count) / Self.sampleRate)
        }
        buffer.append(contentsOf: samples)

        var windows: [AudioWindow] = []
        while Double(buffer.count) / Self.sampleRate >= Self.maxWindow {
            let cut = Self.quietestCut(in: buffer)
            if let window = Self.window(start: bufferStart, samples: Array(buffer[..<cut])) {
                windows.append(window)
            }
            buffer.removeFirst(cut)
            bufferStart += Double(cut) / Self.sampleRate
        }
        return windows
    }

    /// Returns whatever audio is left, or nil if it is silent or too short to be speech.
    public mutating func flush() -> AudioWindow? {
        defer { buffer = [] }
        guard Double(buffer.count) / Self.sampleRate >= Self.minTail else { return nil }
        return Self.window(start: bufferStart, samples: buffer)
    }

    static func window(start: TimeInterval, samples: [Float]) -> AudioWindow? {
        rms(samples[...]) < silenceRMS ? nil : AudioWindow(start: start, samples: samples)
    }

    /// End index of the quietest 100 ms frame between the minimum and maximum window length.
    static func quietestCut(in samples: [Float]) -> Int {
        let lower = Int(minWindow * sampleRate), upper = min(samples.count, Int(maxWindow * sampleRate))
        let starts = stride(from: lower, to: upper - frame + 1, by: frame)
        let best = starts.min { rms(samples[$0..<$0 + frame]) < rms(samples[$1..<$1 + frame]) } ?? upper - frame
        return best + frame / 2
    }

    static func rms(_ samples: ArraySlice<Float>) -> Float {
        guard !samples.isEmpty else { return 0 }
        return (samples.reduce(0) { $0 + $1 * $1 } / Float(samples.count)).squareRoot()
    }
}
