import Cocoa
import XCTest

@testable import KarukanIME

final class CandidateWindowControllerTests: XCTestCase {
    func testPanelFrameStaysInsideRightEdgeOfCurrentScreen() {
        let visibleFrame = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let cursorRect = NSRect(x: 1390, y: 500, width: 8, height: 22)
        let panelSize = NSSize(width: 360, height: 240)

        let frame = CandidateWindowController.panelFrame(
            cursorRect: cursorRect, panelSize: panelSize, visibleFrame: visibleFrame)

        XCTAssertEqual(frame.width, 360)
        XCTAssertGreaterThanOrEqual(frame.minX, visibleFrame.minX)
        XCTAssertLessThanOrEqual(frame.maxX, visibleFrame.maxX)
    }

    func testPanelFrameStaysInsideLeftEdgeOfCurrentScreen() {
        let visibleFrame = NSRect(x: -1280, y: 0, width: 1280, height: 800)
        let cursorRect = NSRect(x: -1278, y: 400, width: 8, height: 22)
        let panelSize = NSSize(width: 300, height: 180)

        let frame = CandidateWindowController.panelFrame(
            cursorRect: cursorRect, panelSize: panelSize, visibleFrame: visibleFrame)

        XCTAssertEqual(frame.width, 300)
        XCTAssertGreaterThanOrEqual(frame.minX, visibleFrame.minX)
        XCTAssertLessThanOrEqual(frame.maxX, visibleFrame.maxX)
    }

    func testPanelFrameShrinksWhenCandidateTextIsWiderThanScreen() {
        let visibleFrame = NSRect(x: 1920, y: 0, width: 640, height: 480)
        let cursorRect = NSRect(x: 2500, y: 300, width: 8, height: 22)
        let panelSize = NSSize(width: 900, height: 160)

        let frame = CandidateWindowController.panelFrame(
            cursorRect: cursorRect, panelSize: panelSize, visibleFrame: visibleFrame)

        XCTAssertLessThan(frame.width, panelSize.width)
        XCTAssertGreaterThanOrEqual(frame.minX, visibleFrame.minX)
        XCTAssertLessThanOrEqual(frame.maxX, visibleFrame.maxX)
    }

    func testPanelFrameFlipsAboveCursorNearBottomEdge() {
        let visibleFrame = NSRect(x: 0, y: 0, width: 1440, height: 900)
        let cursorRect = NSRect(x: 100, y: 40, width: 8, height: 22)
        let panelSize = NSSize(width: 240, height: 180)

        let frame = CandidateWindowController.panelFrame(
            cursorRect: cursorRect, panelSize: panelSize, visibleFrame: visibleFrame)

        XCTAssertGreaterThanOrEqual(frame.minY, cursorRect.maxY)
        XCTAssertGreaterThanOrEqual(frame.minY, visibleFrame.minY)
        XCTAssertLessThanOrEqual(frame.maxY, visibleFrame.maxY)
    }
}
