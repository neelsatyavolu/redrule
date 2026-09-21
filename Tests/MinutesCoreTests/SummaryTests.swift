import Foundation
import Testing
@testable import MinutesCore

@Suite struct SummaryTests {
    let json = """
    {"title":"Launch sync","tldr":"Ship Friday.","sections":[{"heading":"Timeline","bullets":["QA ends Thursday"]}],
     "decisions":["Ship Friday"],"action_items":[{"owner":"Sam","task":"Write release notes"},{"owner":"","task":"Book room"}]}
    """

    @Test func parsesPlainJson() throws {
        let note = try SummaryParser.parse(json)
        #expect(note.title == "Launch sync")
        #expect(note.sections == [NoteSection(heading: "Timeline", bullets: ["QA ends Thursday"])])
        #expect(note.actionItems == [ActionItem(owner: "Sam", task: "Write release notes"), ActionItem(owner: nil, task: "Book room")])
    }

    @Test func parsesJsonWrappedInProseAndFences() throws {
        let note = try SummaryParser.parse("Here you go:\n```json\n\(json)\n```\nHope that helps")
        #expect(note.tldr == "Ship Friday.")
    }

    @Test func rejectsNonJson() {
        #expect(throws: SummaryError.self) { try SummaryParser.parse("I could not summarise this.") }
    }

    @Test func rendersMarkdown() throws {
        let markdown = try SummaryParser.parse(json).markdown
        #expect(markdown == """
        # Launch sync

        Ship Friday.

        ## Timeline
        - QA ends Thursday

        ## Decisions
        - Ship Friday

        ## Action items
        - [ ] **Sam** — Write release notes
        - [ ] Book room

        """)
    }

    @Test func markdownOmitsEmptySections() {
        let note = MeetingNote(title: "T", tldr: "S", sections: [], decisions: [], actionItems: [])
        #expect(note.markdown == "# T\n\nS\n")
    }

    @Test func schemaRequiresEveryTopLevelKey() throws {
        let schema = SummaryPrompt.schema
        let required = try #require(schema["required"] as? [String])
        #expect(Set(required) == ["title", "tldr", "sections", "decisions", "action_items"])
        #expect(JSONSerialization.isValidJSONObject(schema))
    }

    @Test func codexStreamConcatenatesDeltas() throws {
        let sse = """
        event: response.output_text.delta
        data: {"type":"response.output_text.delta","delta":"Hel"}

        data: {"type":"response.output_text.delta","delta":"lo"}

        data: [DONE]
        """
        #expect(try CodexStreamParser.outputText(from: sse) == "Hello")
    }

    @Test func codexStreamPrefersDoneText() throws {
        let sse = """
        data: {"type":"response.output_text.delta","delta":"Hel"}

        data: {"type":"response.output_text.done","text":"Hello!"}
        """
        #expect(try CodexStreamParser.outputText(from: sse) == "Hello!")
    }

    @Test func codexStreamSurfacesErrors() {
        let sse = #"data: {"type":"error","error":{"message":"rate limited"}}"#
        #expect(throws: SummaryError.self) { try CodexStreamParser.outputText(from: sse) }
    }
}
