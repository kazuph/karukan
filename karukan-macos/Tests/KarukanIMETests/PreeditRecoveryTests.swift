import Cocoa
import XCTest

@testable import KarukanIME

final class PreeditRecoveryTests: XCTestCase {
    private func recover(_ preedit: inout DisplayedPreedit, in view: NSTextView) -> Bool {
        preedit.recover(
            insertText: { view.insertText($0, replacementRange: NSRange(location: NSNotFound, length: 0)) },
            clearMarkedText: {
                view.setMarkedText(
                    "", selectedRange: NSRange(location: 0, length: 0),
                    replacementRange: NSRange(location: NSNotFound, length: 0))
            })
    }

    func testLostResponseCommitsVisibleTextAndConsumesReturn() {
        let view = NSTextView()
        view.string = "前文"
        view.setSelectedRange(NSRange(location: view.string.utf16.count, length: 0))
        view.setMarkedText(
            "にほんご", selectedRange: NSRange(location: 4, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0))
        var preedit = DisplayedPreedit(text: "にほんご")

        let consumed = recover(&preedit, in: view)
        if !consumed { view.insertNewline(nil) }

        XCTAssertEqual(view.string, "前文にほんご")
        XCTAssertFalse(view.hasMarkedText())
        XCTAssertTrue(consumed)
        XCTAssertEqual(preedit.text, "")
        _ = recover(&preedit, in: view)
        XCTAssertEqual(view.string, "前文にほんご", "Recovery must not commit twice")
    }

    func testLostResponseConsumesOtherKeysWhenVisibleTextWasCommitted() {
        let view = NSTextView()
        view.setMarkedText(
            "日本語", selectedRange: NSRange(location: 3, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0))
        var preedit = DisplayedPreedit(text: "日本語")
        let consumed = recover(&preedit, in: view)
        if !consumed { view.insertText("a", replacementRange: NSRange(location: NSNotFound, length: 0)) }
        XCTAssertTrue(consumed)
        XCTAssertEqual(view.string, "日本語")
    }

    func testEmptyPreeditStillPassesReturnThrough() {
        let view = NSTextView()
        view.string = "確定済み"
        view.setSelectedRange(NSRange(location: view.string.utf16.count, length: 0))
        var preedit = DisplayedPreedit()
        let consumed = recover(&preedit, in: view)
        if !consumed { view.insertNewline(nil) }
        XCTAssertFalse(consumed)
        XCTAssertEqual(view.string, "確定済み\n")
    }
}
