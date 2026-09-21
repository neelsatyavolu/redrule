import AVFoundation
import ScreenCaptureKit

enum AudioFormat {
    static let sampleRate: Double = 16_000
    static let transcription = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: sampleRate, channels: 1, interleaved: false)!
}

enum CaptureError: LocalizedError {
    case noDisplay
    case noMicrophone

    var errorDescription: String? {
        switch self {
        case .noDisplay: "No display was found to capture system audio from."
        case .noMicrophone: "No microphone is available."
        }
    }
}

/// Converts arbitrary PCM buffers to 16 kHz mono floats, keeping converter state between buffers.
final class Resampler {
    private var converter: AVAudioConverter?
    private var inputFormat: AVAudioFormat?

    func convert(_ buffer: AVAudioPCMBuffer) -> [Float] {
        if inputFormat != buffer.format {
            inputFormat = buffer.format
            converter = AVAudioConverter(from: buffer.format, to: AudioFormat.transcription)
        }
        guard let converter, buffer.frameLength > 0 else { return [] }

        let ratio = AudioFormat.sampleRate / buffer.format.sampleRate
        let capacity = AVAudioFrameCount(Double(buffer.frameLength) * ratio) + 64
        guard let output = AVAudioPCMBuffer(pcmFormat: AudioFormat.transcription, frameCapacity: capacity) else { return [] }

        var delivered = false
        var error: NSError?
        converter.convert(to: output, error: &error) { _, status in
            if delivered {
                status.pointee = .noDataNow
                return nil
            }
            delivered = true
            status.pointee = .haveData
            return buffer
        }
        guard error == nil, let channel = output.floatChannelData?[0] else { return [] }
        return Array(UnsafeBufferPointer(start: channel, count: Int(output.frameLength)))
    }
}

/// Microphone capture through AVAudioEngine.
final class MicCapture {
    private let engine = AVAudioEngine()
    private let resampler = Resampler()
    private var observer: NSObjectProtocol?

    func start(onSamples: @escaping ([Float]) -> Void) throws {
        try installTapAndRun(onSamples)
        // Plugging in headphones or AirPods changes the input format; the tap has to be rebuilt.
        observer = NotificationCenter.default.addObserver(forName: .AVAudioEngineConfigurationChange, object: engine, queue: .main) { [weak self] _ in
            guard let self else { return }
            self.engine.inputNode.removeTap(onBus: 0)
            try? self.installTapAndRun(onSamples)
        }
    }

    private func installTapAndRun(_ onSamples: @escaping ([Float]) -> Void) throws {
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0 else { throw CaptureError.noMicrophone }
        input.installTap(onBus: 0, bufferSize: 4096, format: format) { [resampler] buffer, _ in
            let samples = resampler.convert(buffer)
            if !samples.isEmpty { onSamples(samples) }
        }
        engine.prepare()
        try engine.start()
    }

    func stop() {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        engine.inputNode.removeTap(onBus: 0)
        engine.stop()
    }
}

/// System audio capture through ScreenCaptureKit. Audio from this app is excluded.
final class SystemAudioCapture: NSObject, SCStreamOutput, SCStreamDelegate {
    private var stream: SCStream?
    private let resampler = Resampler()
    private let queue = DispatchQueue(label: "minutes.system-audio")
    private var onSamples: (([Float]) -> Void)?
    private var onStopped: ((Error) -> Void)?

    func start(onSamples: @escaping ([Float]) -> Void, onStopped: @escaping (Error) -> Void) async throws {
        queue.sync {
            self.onSamples = onSamples
            self.onStopped = onStopped
        }

        let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
        guard let display = content.displays.first else { throw CaptureError.noDisplay }

        let configuration = SCStreamConfiguration()
        configuration.capturesAudio = true
        configuration.excludesCurrentProcessAudio = true
        configuration.sampleRate = 48_000
        configuration.channelCount = 1
        // Video is required by the API; keep it as cheap as possible.
        configuration.width = 2
        configuration.height = 2
        configuration.minimumFrameInterval = CMTime(value: 1, timescale: 1)

        let stream = SCStream(filter: SCContentFilter(display: display, excludingWindows: []), configuration: configuration, delegate: self)
        try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: queue)
        try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: queue)
        try await stream.startCapture()
        self.stream = stream
    }

    func stop() async {
        // Clear first so a late "stopped" callback is not reported as an error for a finished recording.
        queue.sync {
            onSamples = nil
            onStopped = nil
        }
        try? await stream?.stopCapture()
        stream = nil
    }

    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio, sampleBuffer.isValid,
              var description = sampleBuffer.formatDescription?.audioStreamBasicDescription,
              let format = AVAudioFormat(streamDescription: &description)
        else { return }
        // The buffer list is only valid inside this closure, so resample before leaving it.
        let samples = try? sampleBuffer.withAudioBufferList { list, _ -> [Float] in
            guard let buffer = AVAudioPCMBuffer(pcmFormat: format, bufferListNoCopy: list.unsafePointer) else { return [] }
            return resampler.convert(buffer)
        }
        if let samples, !samples.isEmpty { onSamples?(samples) }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        onStopped?(error)
    }
}
