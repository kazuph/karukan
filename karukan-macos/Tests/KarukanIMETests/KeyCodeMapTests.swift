import Cocoa
import XCTest

@testable import KarukanIME

final class KeyCodeMapTests: XCTestCase {
    func testPrintableAscii() {
        let event = KeyCodeMap.translate(
            keyCode: 0, characters: "a", charactersIgnoringModifiers: "a", flags: [])
        XCTAssertEqual(event?.keysym, 0x61)
        XCTAssertEqual(event?.modifiers.shift, false)
    }

    func testShiftedLetter() {
        let event = KeyCodeMap.translate(
            keyCode: 0, characters: "A", charactersIgnoringModifiers: "A", flags: [.shift])
        XCTAssertEqual(event?.keysym, 0x41)
        XCTAssertEqual(event?.modifiers.shift, true)
    }

    func testShiftedPunctuation() {
        // IMK key events resolve Shift only in `characters`: Shift+/ comes
        // in as characters="?" but charactersIgnoringModifiers="/". The
        // shifted form must win or ？ becomes ・.
        let event = KeyCodeMap.translate(
            keyCode: 44, characters: "?", charactersIgnoringModifiers: "/", flags: [.shift])
        XCTAssertEqual(event?.keysym, 0x3f)
        XCTAssertEqual(event?.modifiers.shift, true)
    }

    func testControlKeyFallsBackToIgnoringModifiers() {
        // Ctrl+A: `characters` is the control character U+0001; the engine
        // wants the plain key plus the control flag (like fcitx5 sends).
        let event = KeyCodeMap.translate(
            keyCode: 0, characters: "\u{01}", charactersIgnoringModifiers: "a",
            flags: [.control])
        XCTAssertEqual(event?.keysym, 0x61)
        XCTAssertEqual(event?.modifiers.control, true)
    }

    func testOptionGlyphFallsBackToIgnoringModifiers() {
        // Option+a: `characters` is "å"; fall back to the plain key.
        let event = KeyCodeMap.translate(
            keyCode: 0, characters: "å", charactersIgnoringModifiers: "a", flags: [.option])
        XCTAssertEqual(event?.keysym, 0x61)
        XCTAssertEqual(event?.modifiers.alt, true)
    }

    func testSpace() {
        let event = KeyCodeMap.translate(
            keyCode: 49, characters: " ", charactersIgnoringModifiers: " ", flags: [])
        XCTAssertEqual(event?.keysym, 0x20)
    }

    func testReturnKey() {
        let event = KeyCodeMap.translate(
            keyCode: 36, characters: "\r", charactersIgnoringModifiers: "\r", flags: [])
        XCTAssertEqual(event?.keysym, 0xff0d)
    }

    func testEscape() {
        let event = KeyCodeMap.translate(
            keyCode: 53, characters: "\u{1b}", charactersIgnoringModifiers: "\u{1b}", flags: [])
        XCTAssertEqual(event?.keysym, 0xff1b)
    }

    func testBackspace() {
        let event = KeyCodeMap.translate(
            keyCode: 51, characters: "\u{7f}", charactersIgnoringModifiers: "\u{7f}", flags: [])
        XCTAssertEqual(event?.keysym, 0xff08)
    }

    func testArrowKeys() {
        for (keyCode, keysym) in [(123, 0xff51), (124, 0xff53), (125, 0xff54), (126, 0xff52)] {
            XCTAssertEqual(
                KeyCodeMap.translate(
                    keyCode: UInt16(keyCode), characters: nil,
                    charactersIgnoringModifiers: nil, flags: []
                )?.keysym,
                UInt32(keysym))
        }
    }

    func testControlModifier() {
        let event = KeyCodeMap.translate(
            keyCode: 0, characters: "\u{0c}", charactersIgnoringModifiers: "l",
            flags: [.control, .shift])
        XCTAssertEqual(event?.keysym, 0x6c)
        XCTAssertEqual(event?.modifiers.control, true)
        XCTAssertEqual(event?.modifiers.shift, true)
    }

    func testFunctionKeys() {
        // F6-F10 carry the Google日本語入力 transliteration shortcuts
        // (ひらがな/全角カタカナ/半角/全角英数/半角英数).
        for (keyCode, keysym) in [
            (96, 0xffc2),  // F5
            (97, 0xffc3),  // F6
            (98, 0xffc4),  // F7
            (100, 0xffc5),  // F8
            (101, 0xffc6),  // F9
            (109, 0xffc7),  // F10
        ] {
            XCTAssertEqual(
                KeyCodeMap.translate(
                    keyCode: UInt16(keyCode), characters: nil,
                    charactersIgnoringModifiers: nil, flags: []
                )?.keysym,
                UInt32(keysym))
        }
    }

    func testCtrlSemicolonTransliterationKey() {
        // Ctrl+; → 半角. `;` has no ASCII control-character mapping, so
        // IMK delivers the plain character with the control flag set.
        let event = KeyCodeMap.translate(
            keyCode: 41, characters: ";", charactersIgnoringModifiers: ";", flags: [.control])
        XCTAssertEqual(event?.keysym, 0x3b)
        XCTAssertEqual(event?.modifiers.control, true)
    }

    func testCtrlColonJisTransliterationKey() {
        // JIS `:` key (keyCode 39, shared with US `'`). With Control held,
        // `characters` may be empty/non-ASCII; charactersIgnoringModifiers
        // still resolves ":" on a JIS layout.
        let event = KeyCodeMap.translate(
            keyCode: 39, characters: ":", charactersIgnoringModifiers: ":", flags: [.control])
        XCTAssertEqual(event?.keysym, 0x3a)
        XCTAssertEqual(event?.modifiers.control, true)

        // Same physical event where `characters` got mangled into a
        // non-ASCII/empty form must still resolve via the fallback.
        let fallback = KeyCodeMap.translate(
            keyCode: 39, characters: nil, charactersIgnoringModifiers: ":", flags: [.control])
        XCTAssertEqual(fallback?.keysym, 0x3a)
        XCTAssertEqual(fallback?.modifiers.control, true)
    }

    func testCtrlApostropheUsTransliterationKey() {
        // US `'` key (keyCode 39) + Control → 半角英数.
        let event = KeyCodeMap.translate(
            keyCode: 39, characters: "'", charactersIgnoringModifiers: "'", flags: [.control])
        XCTAssertEqual(event?.keysym, 0x27)
        XCTAssertEqual(event?.modifiers.control, true)
    }

    func testCtrlShiftSemicolonResolvesColon() {
        // US layout: Ctrl+Shift+; produces ":" — the 半角英数 shortcut.
        let event = KeyCodeMap.translate(
            keyCode: 41, characters: ":", charactersIgnoringModifiers: ";",
            flags: [.control, .shift])
        XCTAssertEqual(event?.keysym, 0x3a)
        XCTAssertEqual(event?.modifiers.control, true)
        XCTAssertEqual(event?.modifiers.shift, true)
    }

    func testJisYenSignIsTranslated() {
        // JIS 円キー: both `characters` and `charactersIgnoringModifiers`
        // are U+00A5. ASCII-only translation returned nil, so IMK replaced
        // marked text with ¥ while the engine still held てすと.
        let event = KeyCodeMap.translate(
            keyCode: 93, characters: "¥", charactersIgnoringModifiers: "¥", flags: [])
        XCTAssertEqual(event?.keysym, 0x00A5)
        XCTAssertEqual(event?.modifiers.shift, false)
        XCTAssertEqual(event?.modifiers.alt, false)
    }

    func testNonAsciiNotTranslated() {
        // Kana input layouts produce non-ASCII characters; unsupported.
        XCTAssertNil(
            KeyCodeMap.translate(
                keyCode: 0, characters: "あ", charactersIgnoringModifiers: "あ", flags: []))
        XCTAssertNil(
            KeyCodeMap.translate(
                keyCode: 0, characters: nil, charactersIgnoringModifiers: nil, flags: []))
    }
}

final class Utf16ConversionTests: XCTestCase {
    func testAsciiOffsets() {
        XCTAssertEqual(utf16Offset(ofScalarOffset: 2, in: "abc"), 2)
    }

    func testJapaneseOffsets() {
        XCTAssertEqual(utf16Offset(ofScalarOffset: 2, in: "かきく"), 2)
    }

    func testSurrogatePairOffsets() {
        // 𛀗 (hentaigana) is a surrogate pair in UTF-16: 1 scalar == 2 units.
        XCTAssertEqual(utf16Offset(ofScalarOffset: 1, in: "𛀗か"), 2)
        XCTAssertEqual(utf16Offset(ofScalarOffset: 2, in: "𛀗か"), 3)
    }

    func testOffsetClamping() {
        XCTAssertEqual(utf16Offset(ofScalarOffset: 100, in: "かき"), 2)
        XCTAssertEqual(utf16Offset(ofScalarOffset: -1, in: "かき"), 0)
    }

    func testRange() {
        let range = utf16Range(of: 1..<3, in: "𛀗かき")
        XCTAssertEqual(range, NSRange(location: 2, length: 2))
    }
}
