import Foundation
import Testing
@testable import MinutesCore

@Suite struct AudioWindowerTests {
    private func tone(seconds: Double, amplitude: Float = 0.2) -> [Float] {
        (0..<Int(seconds * 16_000)).map { amplitude * sin(Float($0) * 0.05) }
    }

    @Test func holdsAudioUntilMaxWindow() {
        var windower = AudioWindower()
        let windows = windower.append(tone(seconds: 10), endingAt: 10)
        #expect(windows.isEmpty)
    }

    @Test func cutsAtQuietestPointInsideSearchRange() throws {
        var windower = AudioWindower()
        let audio = tone(seconds: 14) + [Float](repeating: 0, count: 8_000) + tone(seconds: 4)
        let windows = windower.append(audio, endingAt: 18.5)
        let window = try #require(windows.first)
        #expect(windows.count == 1)
        #expect(window.start == 0)
        let cutSeconds = Double(window.samples.count) / 16_000
        #expect(cutSeconds >= 14 && cutSeconds <= 14.5)
    }

    @Test func remainderKeepsItsTimeline() throws {
        var windower = AudioWindower()
        let first = windower.append(tone(seconds: 18), endingAt: 18)
        let cut = Double(try #require(first.first).samples.count) / 16_000
        let tail = windower.flush()
        let flushed = try #require(tail)
        #expect(abs(flushed.start - cut) < 0.001)
        let again = windower.flush()
        #expect(again == nil)
    }

    @Test func startReflectsWallClockOfFirstSample() throws {
        var windower = AudioWindower()
        _ = windower.append(tone(seconds: 2), endingAt: 32)
        let tail = windower.flush()
        #expect(tail?.start == 30)
    }

    @Test func silentWindowsAreDropped() {
        var windower = AudioWindower()
        let windows = windower.append([Float](repeating: 0.0001, count: 18 * 16_000), endingAt: 18)
        let tail = windower.flush()
        #expect(windows.isEmpty)
        #expect(tail == nil)
    }

    @Test func tinyTailIsDropped() {
        var windower = AudioWindower()
        _ = windower.append(tone(seconds: 0.2), endingAt: 0.2)
        let tail = windower.flush()
        #expect(tail == nil)
    }
}
