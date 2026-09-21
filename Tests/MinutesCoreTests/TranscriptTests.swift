import Testing
@testable import MinutesCore

@Suite struct TranscriptTests {
    private func seg(_ speaker: Speaker, _ start: Double, _ end: Double, _ text: String) -> TranscriptSegment {
        TranscriptSegment(speaker: speaker, start: start, end: end, text: text)
    }

    @Test func interleavesByStartTime() {
        let merged = TranscriptMerger.merge([seg(.them, 10, 14, "second"), seg(.me, 0, 4, "first")])
        #expect(merged.map(\.text) == ["first", "second"])
    }

    @Test func joinsAdjacentSegmentsFromSameSpeaker() {
        let merged = TranscriptMerger.merge([seg(.me, 0, 4, "hello"), seg(.me, 4.5, 8, "there"), seg(.them, 9, 12, "hi")])
        #expect(merged == [seg(.me, 0, 8, "hello there"), seg(.them, 9, 12, "hi")])
    }

    @Test func dropsEmptySegments() {
        #expect(TranscriptMerger.merge([seg(.me, 0, 4, "  ")]).isEmpty)
    }

    @Test func dropsMicEchoOfSystemAudio() {
        let merged = TranscriptMerger.merge([
            seg(.them, 0, 15, "we should ship the release on friday morning"),
            seg(.me, 0, 15, "we should ship the release on friday morning"),
            seg(.me, 15, 30, "sounds good to me"),
        ])
        #expect(merged.map(\.speaker) == [.them, .me])
        #expect(merged.last?.text == "sounds good to me")
    }

    @Test func rendersTimestampedLines() {
        let text = TranscriptMerger.render([seg(.me, 65, 70, "hello"), seg(.them, 3700, 3705, "bye")])
        #expect(text == "[01:05] Me: hello\n[1:01:40] Them: bye")
    }

    @Test func chunkerKeepsShortTextWhole() {
        #expect(TranscriptChunker.chunks("a\nb", budget: 100) == ["a\nb"])
    }

    @Test func chunkerSplitsOnLineBoundaries() {
        let chunks = TranscriptChunker.chunks("aaaa\nbbbb\ncccc", budget: 9)
        #expect(chunks == ["aaaa\nbbbb", "cccc"])
    }

    @Test func chunkerKeepsOversizedLineAlone() {
        let chunks = TranscriptChunker.chunks("aaaaaaaaaaaa\nbb", budget: 5)
        #expect(chunks == ["aaaaaaaaaaaa", "bb"])
    }
}
