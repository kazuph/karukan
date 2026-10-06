import Cocoa
import InputMethodKit

/// Thin InputMethodKit adapter for the karukan engine.
///
/// All IME state (Empty → Composing → Conversion, romaji conversion,
/// candidates, learning) lives in karukan-imserver; this controller only
/// translates key events and applies the resulting UI actions, mirroring
/// the fcitx5 addon (karukan.cpp).
@objc(KarukanInputController)
class KarukanInputController: IMKInputController {
    static let candidateWindow = CandidateWindowController()

    private var displayedPreedit = DisplayedPreedit()
    private var hasPreedit: Bool { !displayedPreedit.text.isEmpty }
    private lazy var inputSession = EngineInputSession(engine: engineClient)

    /// Diagnostics for the "IME stays selected but no keys arrive"
    /// failure: when macOS last activated this client session, and how
    /// many keyDown events reached `handle` since then.
    private var activatedAt: Date?
    private var keyCount = 0

    // MARK: - Event handling

    override func recognizedEvents(_ sender: Any!) -> Int {
        Int(NSEvent.EventTypeMask.keyDown.rawValue)
    }

    override func handle(_ event: NSEvent!, client sender: Any!) -> Bool {
        guard let event else { return false }
        guard event.type == .keyDown else { return false }
        keyCount += 1
        guard let client = sender as? (any IMKTextInput) else { return false }

        let deadline = EngineClient.synchronousDeadline()

        let flags = event.modifierFlags.intersection(.deviceIndependentFlagsMask)
        // Never swallow Command shortcuts.
        if flags.contains(.command) { return false }

        // Consume a successful かな switch even when already in hiragana mode.
        // A lost response follows the same visible-text recovery as other keys.
        if event.keyCode == KeyCodeMap.kanaKeyCode {
            return inputSession.handleKanaSwitch(
                deadline: deadline,
                apply: { self.apply(actions: $0, client: client) },
                recover: { self.resyncAfterLostResponse(client: client) })
        }

        // JIS 英数 key: flush pending composition so preedit doesn't linger
        // after macOS switches to the English input source.
        if event.keyCode == KeyCodeMap.eisuKeyCode {
            flushComposition(client: client, deadline: deadline)
            return false
        }

        guard let key = KeyCodeMap.translate(event: event) else { return false }

        // Refresh the conversion context while no composition is active
        // (mirrors the fcitx5 addon, which captures surrounding text in the
        // Empty state). Queued before process_key on the same pipe, so the
        // engine sees it first. Skipped for function/navigation keysyms
        // (0xff00 range): they can't start a composition, and the three
        // synchronous client IPCs in sendSurroundingText would otherwise
        // fire on every arrow-key repeat.
        if !hasPreedit && !inputSession.isProcessingAsync && key.keysym < 0xff00 {
            sendSurroundingText(client: client)
        }

        return inputSession.handle(
            key, deadline: deadline, hasPreedit: { self.hasPreedit },
            apply: { self.apply(actions: $0, client: client) },
            recover: { self.resyncAfterLostResponse(client: client) })
    }

    /// Commit the last rendered text before resetting the engine. A late response
    /// is discarded by the transport; reset is queued after that in-flight request.
    @discardableResult
    private func resyncAfterLostResponse(client: any IMKTextInput) -> Bool {
        NSLog("KarukanIME: lost engine response, preserving visible preedit and resetting")
        let consumed = displayedPreedit.recover(
            insertText: {
                client.insertText($0, replacementRange: NSRange(location: NSNotFound, length: 0))
            },
            clearMarkedText: {
                client.setMarkedText(
                    NSAttributedString(string: ""),
                    selectionRange: NSRange(location: 0, length: 0),
                    replacementRange: NSRange(location: NSNotFound, length: 0))
            })
        Self.candidateWindow.hide()
        engineClient.resetAsync()
        return consumed
    }

    // MARK: - Lifecycle

    override func activateServer(_ sender: Any!) {
        activatedAt = Date()
        keyCount = 0
        let clientBundleID = (sender as? (any IMKTextInput))?.bundleIdentifier() ?? "unknown"
        NSLog("KarukanIME: activated (client=\(clientBundleID))")
        super.activateServer(sender)
    }

    override func deactivateServer(_ sender: Any!) {
        if let activatedAt {
            let elapsedMs = Int(Date().timeIntervalSince(activatedAt) * 1000)
            NSLog("KarukanIME: deactivated after \(elapsedMs)ms, keys=\(keyCount)")
        } else {
            NSLog("KarukanIME: deactivated (no activateServer seen), keys=\(keyCount)")
        }
        self.activatedAt = nil
        // Mozc-style: commit the pending preedit on focus loss, then
        // persist what the user taught us.
        if let client = sender as? (any IMKTextInput) {
            flushComposition(client: client)
        } else {
            inputSession.flushPendingComposition()
            Self.candidateWindow.hide()
        }
        engineClient.saveLearningAsync()
        super.deactivateServer(sender)
    }

    override func commitComposition(_ sender: Any!) {
        if let client = sender as? (any IMKTextInput) {
            flushComposition(client: client)
        } else {
            inputSession.flushPendingComposition()
            Self.candidateWindow.hide()
        }
    }

    /// Commit any pending composition via the engine and apply the cleanup
    /// actions it emits (clear preedit, hide candidates/aux).
    private func flushComposition(
        client: any IMKTextInput, deadline: DispatchTime = EngineClient.synchronousDeadline()
    ) {
        inputSession.flush(
            deadline: deadline, hasPreedit: { self.hasPreedit },
            apply: { self.apply(actions: $0, client: client) },
            recover: { self.resyncAfterLostResponse(client: client) })
    }

    // MARK: - Applying engine actions

    private func apply(actions: [EngineAction], client: any IMKTextInput) {
        // The engine emits ShowCandidates before UpdateAux. Fold aux changes
        // in first (deferring their render when a candidate update follows)
        // so the panel is rendered once per batch, not once for the
        // candidates and again for the aux footer.
        let updatesCandidates = actions.contains {
            switch $0 {
            case .showCandidates, .hideCandidates: return true
            default: return false
            }
        }
        for action in actions {
            switch action {
            case .updateAux(let text):
                Self.candidateWindow.setAux(text, deferRender: updatesCandidates)
            case .hideAux:
                Self.candidateWindow.setAux(nil, deferRender: updatesCandidates)
            default:
                break
            }
        }

        for action in actions {
            switch action {
            case .commit(let text):
                displayedPreedit.text = ""
                client.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))

            case .updatePreedit(let text, let caret, let attributes):
                setMarkedText(text: text, caret: caret, attributes: attributes, client: client)

            case .showCandidates(let candidates, let cursor, let page, let totalPages):
                // Query the composition anchor (a synchronous IPC into the
                // focused app) only when the panel comes on screen; it
                // doesn't move while the panel stays visible.
                var cursorRect: NSRect?
                Self.candidateWindow.setCandidateSelectHandler { [weak self] pageIndex in
                    guard let self else { return }
                    self.selectCandidateFromWindow(pageIndex: pageIndex, client: client)
                }
                if !Self.candidateWindow.isVisible {
                    var lineHeightRect = NSRect.zero
                    client.attributes(forCharacterIndex: 0, lineHeightRectangle: &lineHeightRect)
                    cursorRect = lineHeightRect
                }
                Self.candidateWindow.show(
                    candidates: candidates,
                    cursor: cursor,
                    page: page,
                    totalPages: totalPages,
                    cursorRect: cursorRect
                )

            case .hideCandidates:
                Self.candidateWindow.hide()

            case .updateAux, .hideAux:
                break  // applied above
            }
        }
    }

    /// Send the text left of the cursor to the engine as conversion
    /// context. Gated on `selectedRange` only: `client.length()` is the
    /// least-implemented part of IMKTextInput (it returns 0 even in apps
    /// whose `attributedSubstring` works fine), and the request below is
    /// capped to 40 UTF-16 units anyway, so document size doesn't matter.
    /// Whether a client supports this at all is app-dependent (Cocoa text
    /// views do; Electron/Chromium/terminals mostly don't), so the skip
    /// reasons are logged for dogfooding visibility.
    private func sendSurroundingText(client: any IMKTextInput) {
        // When capture isn't possible, CLEAR the engine's context rather
        // than skipping: leaving the context from a previous cursor
        // position in place makes the engine condition on (and display)
        // text that is no longer left of the cursor. No context beats a
        // wrong one. selectedRange flakiness is per-keystroke in some
        // apps, so this also self-heals on the next successful capture.
        let selected = client.selectedRange()
        guard selected.location != NSNotFound, selected.location > 0 else {
            NSLog("KarukanIME: surrounding text cleared (no usable selection)")
            engineClient.setSurroundingTextAsync(text: "", cursorPos: 0)
            return
        }

        let maxContextUTF16 = 40  // engine truncates further per its config
        let start = max(0, selected.location - maxContextUTF16)
        let range = NSRange(location: start, length: selected.location - start)
        // string(from:actualRange:) rather than attributedSubstring(from:):
        // it's the IMKTextInput document-access method clients actually
        // implement (azooKey-Desktop settled on the same call).
        var actualRange = NSRange()
        guard let leftContext = client.string(from: range, actualRange: &actualRange),
            !leftContext.isEmpty
        else {
            NSLog("KarukanIME: surrounding text cleared (string(from:) unavailable)")
            engineClient.setSurroundingTextAsync(text: "", cursorPos: 0)
            return
        }

        NSLog("KarukanIME: surrounding text captured (\(leftContext.count) chars)")
        engineClient.setSurroundingTextAsync(
            text: leftContext,
            cursorPos: leftContext.unicodeScalars.count
        )
    }

    private func setMarkedText(
        text: String, caret: Int, attributes: [PreeditAttr], client: any IMKTextInput
    ) {
        displayedPreedit.text = text
        guard !text.isEmpty else {
            client.setMarkedText(
                NSAttributedString(string: ""),
                selectionRange: NSRange(location: 0, length: 0),
                replacementRange: NSRange(location: NSNotFound, length: 0)
            )
            return
        }

        let attributed = NSMutableAttributedString(
            string: text,
            attributes: [.underlineStyle: NSUnderlineStyle.single.rawValue]
        )
        for attr in attributes {
            guard let range = utf16Range(of: attr.start..<attr.end, in: text) else { continue }
            let style: NSUnderlineStyle
            switch attr.style {
            // The focused/highlighted segment is drawn with a thick
            // underline (the convention azooKey/mac-akaza use for marked
            // text, since background colors are unreliable across apps).
            case "underline_double", "highlight", "reverse":
                style = .thick
            default:
                style = .single
            }
            attributed.addAttribute(.underlineStyle, value: style.rawValue, range: range)
        }

        let caretUTF16 = utf16Offset(ofScalarOffset: caret, in: text)
        client.setMarkedText(
            attributed,
            selectionRange: NSRange(location: caretUTF16, length: 0),
            replacementRange: NSRange(location: NSNotFound, length: 0)
        )
    }

    private func selectCandidateFromWindow(pageIndex: Int, client: any IMKTextInput) {
        inputSession.selectCandidate(
            pageIndex: pageIndex, hasPreedit: { self.hasPreedit },
            apply: { self.apply(actions: $0, client: client) },
            recover: { self.resyncAfterLostResponse(client: client) })
    }
}

// MARK: - Unicode scalar → UTF-16 offset conversion

/// The engine reports positions in Unicode scalar values; IMK APIs take
/// UTF-16 offsets.
func utf16Offset(ofScalarOffset offset: Int, in text: String) -> Int {
    let scalars = text.unicodeScalars
    let clamped = min(max(offset, 0), scalars.count)
    let index = scalars.index(scalars.startIndex, offsetBy: clamped)
    return text.utf16.distance(from: text.utf16.startIndex, to: index)
}

func utf16Range(of scalarRange: Range<Int>, in text: String) -> NSRange? {
    guard scalarRange.lowerBound >= 0, scalarRange.lowerBound <= scalarRange.upperBound else {
        return nil
    }
    let start = utf16Offset(ofScalarOffset: scalarRange.lowerBound, in: text)
    let end = utf16Offset(ofScalarOffset: scalarRange.upperBound, in: text)
    return NSRange(location: start, length: end - start)
}

/// The text actually shown to the client, independent of an in-flight engine request.
struct DisplayedPreedit {
    var text = ""

    @discardableResult
    mutating func recover(insertText: (String) -> Void, clearMarkedText: () -> Void) -> Bool {
        let visibleText = text
        text = ""
        if !visibleText.isEmpty { insertText(visibleText) }
        clearMarkedText()
        return !visibleText.isEmpty
    }
}

/// Key dispatch shared by the IMK adapter and real-engine integration tests.
final class EngineInputSession {
    private let engine: EngineClient
    private var pendingInput: PendingInput?
    private var queuedKeys: [QueuedKey] = []
    private var isApplyingInput = false

    var isProcessingAsync: Bool { pendingInput != nil || !queuedKeys.isEmpty || isApplyingInput }

    init(engine: EngineClient) { self.engine = engine }

    func handleKanaSwitch(
        deadline: DispatchTime = EngineClient.synchronousDeadline(),
        apply: @escaping ([EngineAction]) -> Void, recover: @escaping () -> Bool
    ) -> Bool {
        let key = EngineKeyEvent(keysym: KeyCodeMap.superRKeysym, modifiers: KeyModifiers())
        if isProcessingAsync {
            queuedKeys.append(QueuedKey(key: key, apply: apply, recover: recover))
            return true
        }
        // Always consume: the system must never see keyCode 104 (existing contract).
        guard let result = engine.processKeySync(key, deadline: deadline) else {
            _ = recover()
            return true
        }
        apply(result.actions)
        return true
    }

    func handle(
        _ key: EngineKeyEvent, deadline: DispatchTime = EngineClient.synchronousDeadline(),
        hasPreedit: () -> Bool,
        apply: @escaping ([EngineAction]) -> Void, recover: @escaping () -> Bool
    ) -> Bool {
        if key.modifiers.superKey { return false }
        let input = QueuedKey(key: key, apply: apply, recover: recover)
        if isProcessingAsync {
            queuedKeys.append(input)
            return true
        }
        if hasPreedit() && (key.keysym == 0xff0d || key.keysym == 0xff8d) {
            start(input, preserveKeyOnLoss: false)
            return true
        }
        guard let result = engine.processKeySync(key, deadline: deadline) else { return recover() }
        apply(result.actions)
        return result.consumed
    }

    /// Lifecycle barriers may wait for the entire queue, but share one two-second
    /// budget. If it expires, preserve undelivered printable keys on their original client.
    @discardableResult
    func finishPending(until deadline: DispatchTime) -> Bool {
        let hadPending = isProcessingAsync
        while let pending = pendingInput {
            pending.wait(until: min(deadline, pending.deadline))
            if pending.isReady {
                finish(pending)
            } else {
                preserveAndDiscardQueue(pending)
                break
            }
        }
        return hadPending
    }

    func flush(
        deadline: DispatchTime = EngineClient.synchronousDeadline(), hasPreedit: () -> Bool,
        apply: ([EngineAction]) -> Void, recover: () -> Bool
    ) {
        if finishPending(until: deadline) && !hasPreedit() { return }
        if let result = engine.commitSync(deadline: deadline) { apply(result.actions) } else { _ = recover() }
    }

    func flushPendingComposition() {
        guard let input = pendingInput?.input else { return }
        let deadline = EngineClient.synchronousDeadline()
        finishPending(until: deadline)
        if let result = engine.commitSync(deadline: deadline) {
            input.apply(result.actions)
        } else {
            _ = input.recover()
        }
    }

    func selectCandidate(
        pageIndex: Int, hasPreedit: () -> Bool,
        apply: ([EngineAction]) -> Void, recover: () -> Bool
    ) {
        let deadline = EngineClient.synchronousDeadline()
        if finishPending(until: deadline) && !hasPreedit() { return }
        if let result = engine.selectCandidateSync(pageIndex: pageIndex, deadline: deadline) {
            apply(result.actions)
        } else {
            _ = recover()
        }
    }

    private func start(_ input: QueuedKey, preserveKeyOnLoss: Bool) {
        let pending = PendingInput(input: input, preserveKeyOnLoss: preserveKeyOnLoss)
        pendingInput = pending
        pending.requestID = engine.processKeyAsync(input.key) { [weak self] result in
            pending.resolve(result)
            DispatchQueue.main.async { self?.finish(pending) }
        }
        DispatchQueue.main.asyncAfter(deadline: pending.deadline) { [weak self, weak pending] in
            guard let pending else { return }
            self?.finish(pending)
        }
    }

    private func finish(_ pending: PendingInput) {
        // Claim once on main; late replies and timers cannot apply to newer input.
        guard pendingInput === pending else { return }
        pendingInput = nil
        if let id = pending.requestID { engine.abandonRequest(id) }
        isApplyingInput = true
        if let result = pending.result {
            pending.input.apply(result.actions)
            if !result.consumed { pending.input.preservePrintableKey() }
        } else {
            _ = pending.input.recover()
            if pending.preserveKeyOnLoss { pending.input.preservePrintableKey() }
        }
        isApplyingInput = false
        if !queuedKeys.isEmpty { start(queuedKeys.removeFirst(), preserveKeyOnLoss: true) }
    }

    private func preserveAndDiscardQueue(_ pending: PendingInput) {
        guard pendingInput === pending else { return }
        let remaining = queuedKeys
        queuedKeys.removeAll()
        pendingInput = nil
        if let id = pending.requestID { engine.abandonRequest(id) }
        _ = pending.input.recover()
        if pending.preserveKeyOnLoss { pending.input.preservePrintableKey() }
        for input in remaining { input.preservePrintableKey() }
    }

    private struct QueuedKey {
        let key: EngineKeyEvent
        let apply: ([EngineAction]) -> Void
        let recover: () -> Bool

        func preservePrintableKey() {
            // The translated printable domain is ASCII plus the JIS yen key.
            guard (0x20...0x7e).contains(key.keysym) || key.keysym == 0x00a5,
                !key.modifiers.control, !key.modifiers.superKey,
                let scalar = UnicodeScalar(key.keysym)
            else { return }
            apply([.commit(text: String(scalar))])
        }
    }

    private final class PendingInput {
        let deadline = DispatchTime.now() + EngineClient.enterTimeout
        let input: QueuedKey
        let preserveKeyOnLoss: Bool
        var requestID: Int?
        private let lock = NSLock()
        private let semaphore = DispatchSemaphore(value: 0)
        private var ready = false
        private var value: KeyResult?

        init(input: QueuedKey, preserveKeyOnLoss: Bool) {
            self.input = input
            self.preserveKeyOnLoss = preserveKeyOnLoss
        }

        var isReady: Bool {
            lock.lock()
            defer { lock.unlock() }
            return ready
        }

        func resolve(_ result: KeyResult?) {
            lock.lock()
            value = DispatchTime.now() <= deadline ? result : nil
            ready = true
            lock.unlock()
            semaphore.signal()
        }

        var result: KeyResult? {
            lock.lock()
            defer { lock.unlock() }
            return value
        }

        func wait(until deadline: DispatchTime) {
            lock.lock()
            let alreadyReady = ready
            lock.unlock()
            if !alreadyReady { _ = semaphore.wait(timeout: deadline) }
        }
    }
}
