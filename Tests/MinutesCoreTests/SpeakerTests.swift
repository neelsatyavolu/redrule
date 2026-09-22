import Foundation
import Testing
@testable import MinutesCore

@Suite struct SpeakerTests {
    @Test func legacyTranscriptStillDecodes() throws {
        let data = Data(#"{"speaker":"them","start":0,"end":2,"text":"Hello"}"#.utf8)
        let segment = try JSONDecoder().decode(TranscriptSegment.self, from: data)
        #expect(segment.speakerID == nil)
        #expect(segment.speakerLabel == "Them")
    }

    @Test func distinctSpeakersAreNotMergedAndNamesSurvive() throws {
        let a = TranscriptSegment(speaker: .them, start: 0, end: 2, text: " Hello ", speakerID: "1", speakerName: "Alice")
        let b = TranscriptSegment(speaker: .them, start: 2, end: 3, text: "Hi", speakerID: "2")
        let result = TranscriptMerger.merge([a, b])
        #expect(result.count == 2)
        #expect(result.first?.speakerLabel == "Alice")
        #expect(TranscriptMerger.render(result).contains("Speaker 2: Hi"))
        #expect(try JSONDecoder().decode([TranscriptSegment].self, from: JSONEncoder().encode(result)) == result)
    }

    @Test func wordsFollowSpeakerTurnsAndUnknownStaysUnassigned() {
        let words = [TranscriptWord(text: "Hello", start: 0, end: 0.5), TranscriptWord(text: "there", start: 1, end: 1.5), TranscriptWord(text: "okay", start: 4, end: 4.5)]
        let turns = [SpeakerTurn(id: "1", start: 0, end: 0.7), SpeakerTurn(id: "2", start: 0.8, end: 2)]
        let segments = SpeakerAlignment.segments(words: words, turns: turns, offset: 10)
        #expect(segments.map(\.speakerID) == ["1", "2", nil])
        #expect(segments.map(\.text) == ["Hello", "there", "okay"])
        #expect(segments.first?.start == 10)
    }

    @Test func overlappingSpeakersRemainUnknown() {
        let words = [TranscriptWord(text: "Yes", start: 0, end: 1)]
        let turns = [SpeakerTurn(id: "1", start: 0, end: 1), SpeakerTurn(id: "2", start: 0, end: 1)]
        #expect(SpeakerAlignment.segments(words: words, turns: turns, offset: 0).first?.speakerID == nil)
    }
}
