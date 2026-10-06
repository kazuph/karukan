import Cocoa
import XCTest

@testable import KarukanIME

/// Real engine responses, delayed on the pipe; rendering uses a real Cocoa text view.
final class AsyncEnterTests: XCTestCase {
    private var process: EngineProcess!
    private var client: EngineClient!
    private var session: EngineInputSession!
    private var directory: URL!
    private var view: NSTextView!
    private var preedit = DisplayedPreedit()
    private var commits: [String] = []
    private var recoveries = 0
    private var keyDuringApply: Int?

    private func start(
        delay: Double, keys: [Int] = [0xff0d, 0xff8d], initializeConversion: Bool = false, delayOnce: Bool = false
    ) throws {
        let binary = try XCTUnwrap(TransportTests.serverBinaryPath())
        directory = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        if initializeConversion {
            // Copy public model cache into the isolated HOME; never write through a user-cache symlink.
            let hub = directory.appendingPathComponent(".cache/huggingface/hub")
            try FileManager.default.createDirectory(at: hub, withIntermediateDirectories: true)
            // Default Adaptive initialization loads both models, even for a main-model Space test.
            for name in ["models--togatogah--jinen-v2-small.gguf", "models--togatogah--jinen-v2-xsmall.gguf"] {
                let cache = FileManager.default.homeDirectoryForCurrentUser
                    .appendingPathComponent(".cache/huggingface/hub").appendingPathComponent(name)
                try FileManager.default.copyItem(at: cache, to: hub.appendingPathComponent(name))
            }
        }
        let proxy = directory.appendingPathComponent("delay-server")
        let script = """
            #!/usr/bin/python3
            import json, os, subprocess, sys, time
            env = dict(os.environ, HOME=\(String(reflecting: directory.path)))
            server = subprocess.Popen([\(String(reflecting: binary))], stdin=subprocess.PIPE, stdout=subprocess.PIPE, env=env)
            delayed = False
            for line in sys.stdin:
                request = json.loads(line)
                server.stdin.write(line.encode())
                server.stdin.flush()
                response = server.stdout.readline()
                if request['method'] == 'process_key' and request['params']['keysym'] in \(keys) and (not \(delayOnce ? "True" : "False") or not delayed):
                    delayed = True
                    time.sleep(\(delay))
                sys.stdout.buffer.write(response)
                sys.stdout.buffer.flush()
            server.stdin.close()
            server.wait()
            """
        try script.write(to: proxy, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: proxy.path)
        process = EngineProcess(serverPath: proxy.path)
        client = EngineClient(serverProcess: process, autoInit: false)
        session = EngineInputSession(engine: client)
        view = NSTextView()
        process.start()
        client.startReaderLoop()
        XCTAssertNotNil(client.sendRequestSync(method: "status", params: [:], timeout: 5))
        if initializeConversion {
            let data = try XCTUnwrap(client.sendRequestSync(method: "init", params: [:], timeout: 15))
            _ = try makeProtocolDecoder().decode(InitResult.self, from: data)
        }

    }

    override func tearDownWithError() throws {
        if client != nil {
            XCTAssertNotNil(client.sendRequestSync(method: "status", params: [:], timeout: 15))
            process.stop()
        }
        if directory != nil { try FileManager.default.removeItem(at: directory) }
    }

    private func apply(_ actions: [EngineAction]) {
        XCTAssertTrue(Thread.isMainThread)
        for action in actions {
            switch action {
            case .commit(let text):
                commits.append(text)
                preedit.text = ""
                view.insertText(text, replacementRange: NSRange(location: NSNotFound, length: 0))
                if let key = keyDuringApply {
                    keyDuringApply = nil
                    let start = Date()
                    XCTAssertTrue(handle(key))
                    XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
                }
            case .updatePreedit(let text, let caret, _):
                preedit.text = text
                view.setMarkedText(
                    text, selectedRange: NSRange(location: utf16Offset(ofScalarOffset: caret, in: text), length: 0),
                    replacementRange: NSRange(location: NSNotFound, length: 0))
            default: break
            }
        }
    }

    private func recover() -> Bool {
        recoveries += 1
        XCTAssertTrue(Thread.isMainThread)
        let consumed = preedit.recover(
            insertText: {
                self.commits.append($0)
                self.view.insertText($0, replacementRange: NSRange(location: NSNotFound, length: 0))
            },
            clearMarkedText: {
                self.view.setMarkedText(
                    "", selectedRange: NSRange(location: 0, length: 0),
                    replacementRange: NSRange(location: NSNotFound, length: 0))
            })
        client.resetAsync()
        return consumed
    }

    @discardableResult
    private func handle(_ keysym: Int) -> Bool {
        session.handle(
            EngineKeyEvent(keysym: UInt32(keysym), modifiers: KeyModifiers()),
            hasPreedit: { !self.preedit.text.isEmpty }, apply: apply, recover: recover)
    }

    private func compose() {
        for scalar in "nihongo".unicodeScalars { XCTAssertTrue(handle(Int(scalar.value))) }
        XCTAssertEqual(preedit.text, "にほんご")
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertTrue(view.hasMarkedText())
    }

    func testDelayedReturnIsConsumedImmediatelyAndCommitsOnMain() throws {
        try start(delay: 4)
        compose()
        let start = Date()
        XCTAssertTrue(handle(0xff0d))
        XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
        XCTAssertTrue(view.hasMarkedText(), "Return must leave the preedit visible while waiting")
        let delivered = expectation(description: "real Return response delivered")
        DispatchQueue.main.asyncAfter(deadline: .now() + 4.5) { delivered.fulfill() }
        wait(for: [delivered], timeout: 6)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertEqual(recoveries, 0, "The real response, not timeout recovery, must commit")
        XCTAssertFalse(view.hasMarkedText())
        XCTAssertEqual(preedit.text, "")
    }

    private func runMainLoop(for seconds: Double) {
        let elapsed = expectation(description: "allow real delayed delivery")
        DispatchQueue.main.asyncAfter(deadline: .now() + seconds) { elapsed.fulfill() }
        wait(for: [elapsed], timeout: seconds + 2)
    }

    private func passThrough(_ keysym: Int, consumed: Bool) {
        if !consumed {
            if keysym == 0xff0d || keysym == 0xff8d {
                view.insertNewline(nil)
            } else {
                view.insertText(
                    String(UnicodeScalar(keysym)!), replacementRange: NSRange(location: NSNotFound, length: 0))
            }
        }
    }

    func testDelayedKeypadEnterIsAlsoImmediate() throws {
        try start(delay: 4)
        compose()
        let start = Date()
        let consumed = handle(0xff8d)
        XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
        XCTAssertTrue(consumed)
        passThrough(0xff8d, consumed: consumed)
        XCTAssertTrue(view.hasMarkedText())
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertEqual(recoveries, 0, "The real response, not timeout recovery, must commit")
        XCTAssertFalse(view.hasMarkedText())
    }

    func testTenSecondDeadlinePreservesTextAndDiscardsLateCommit() throws {
        try start(delay: 11)
        compose()
        XCTAssertTrue(handle(0xff0d))
        runMainLoop(for: 9.5)
        XCTAssertTrue(view.hasMarkedText(), "Keep preedit visible before the ten-second deadline")
        XCTAssertEqual(commits, [])
        XCTAssertEqual(recoveries, 0)
        runMainLoop(for: 0.8)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertFalse(view.hasMarkedText())
        runMainLoop(for: 1.5)
        XCTAssertEqual(commits, ["にほんご"], "Late engine commit must be discarded")
        XCTAssertEqual(recoveries, 1, "Recover exactly once after the ten-second deadline")
        XCTAssertTrue(handle(0x61))
        XCTAssertEqual(view.string, "にほんごあ")
    }

    func testFollowingKeyAppliesReadyEnterBeforeNewPreedit() throws {
        try start(delay: 4)
        compose()
        XCTAssertTrue(handle(0xff0d))
        runMainLoop(for: 3)
        XCTAssertTrue(handle(0x61))
        runMainLoop(for: 1.5)
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertEqual(view.string, "にほんごあ")
        runMainLoop(for: 0.5)
        XCTAssertEqual(commits, ["にほんご"], "Queued main callback must not apply Enter twice")
        XCTAssertEqual(preedit.text, "あ")
    }

    func testFollowingKeyIsQueuedImmediatelyWithoutUsingCallbackBudget() throws {
        try start(delay: 4)
        compose()
        XCTAssertTrue(handle(0xff0d))
        let start = Date()
        let consumed = handle(0x61)
        XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
        XCTAssertTrue(consumed)
        passThrough(0x61, consumed: consumed)
        XCTAssertEqual(commits, [])
        XCTAssertEqual(view.string, "にほんご")
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, "にほんごあ")
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertTrue(handle(0x75))
        XCTAssertEqual(view.string, "にほんごあう")
    }

    func testKeysImmediatelyAfterDelayedEnterAreQueuedWithoutLossOrNewline() throws {
        try start(delay: 4, delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        runMainLoop(for: 0.3)
        for key in Array("kanji".unicodeScalars.map { Int($0.value) }) + [0xff0d] {
            let start = Date()
            let consumed = handle(key)
            XCTAssertTrue(consumed)
            XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
            passThrough(key, consumed: consumed)
        }
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, "にほんごかんじ")
        XCTAssertEqual(commits, ["にほんご", "かんじ"])
        XCTAssertEqual(recoveries, 0)
        XCTAssertFalse(view.hasMarkedText())
    }

    func testKeyReceivedDuringActionApplicationIsQueuedImmediately() throws {
        try start(delay: 4, keys: [0x6b], delayOnce: true)
        compose()
        keyDuringApply = 0x6b
        XCTAssertTrue(handle(0xff0d))
        runMainLoop(for: 0.3)
        XCTAssertNil(keyDuringApply, "A real commit response must trigger the concurrent key")
        for key in Array("anji".unicodeScalars.map { Int($0.value) }) + [0xff0d] {
            XCTAssertTrue(handle(key))
        }
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, "にほんごかんじ")
        XCTAssertEqual(commits, ["にほんご", "かんじ"])
        XCTAssertEqual(recoveries, 0)
    }

    func testKeysArrivingWhileQueueKeyIsInFlightStayInOrder() throws {
        try start(delay: 4, keys: [0x6b], delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        XCTAssertTrue(handle(0x6b))
        runMainLoop(for: 0.3)
        for key in Array("anji".unicodeScalars.map { Int($0.value) }) + [0xff0d] {
            let start = Date()
            XCTAssertTrue(handle(key))
            XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
        }
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, "にほんごかんじ")
        XCTAssertEqual(commits, ["にほんご", "かんじ"])
        XCTAssertEqual(recoveries, 0)
    }

    func testEnterDeadlineRecoveryContinuesQueuedKeys() throws {
        try start(delay: 12, delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        for key in Array("kanji".unicodeScalars.map { Int($0.value) }) + [0xff0d] {
            XCTAssertTrue(handle(key))
        }
        runMainLoop(for: 10.3)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        runMainLoop(for: 2.5)
        XCTAssertEqual(view.string, "にほんごかんじ")
        XCTAssertEqual(commits, ["にほんご", "かんじ"])
        XCTAssertEqual(recoveries, 1)
        XCTAssertFalse(view.hasMarkedText())
    }

    func testQueuedKeyDeadlinePreservesThatKeyAndContinuesRemainder() throws {
        try start(delay: 11, keys: [0x6b], delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        XCTAssertTrue(handle(0x6b))
        XCTAssertTrue(handle(0x61))
        XCTAssertTrue(handle(0xff0d))
        runMainLoop(for: 10.4)
        XCTAssertEqual(view.string, "にほんごk")
        XCTAssertEqual(recoveries, 1)
        runMainLoop(for: 1.5)
        XCTAssertEqual(view.string, "にほんごkあ")
        XCTAssertEqual(commits, ["にほんご", "k", "あ"])
        XCTAssertFalse(view.hasMarkedText())
    }

    func testCommandShortcutPassesImmediatelyWithoutEnteringQueue() throws {
        try start(delay: 4, delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        let start = Date()
        XCTAssertFalse(
            session.handle(
                EngineKeyEvent(keysym: 0x61, modifiers: KeyModifiers(superKey: true)),
                hasPreedit: { !self.preedit.text.isEmpty }, apply: apply, recover: recover))
        XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
        XCTAssertTrue(handle(0x75))
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, "にほんごう")
        XCTAssertEqual(recoveries, 0)
    }

    private func assertBarrierPreservesQueuedRomaji(_ barrier: () -> Void) throws {
        try start(delay: 4, delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        for key in Array("kanji".unicodeScalars.map { Int($0.value) }) + [0xff0d] {
            XCTAssertTrue(handle(key))
        }
        let start = Date()
        barrier()
        XCTAssertLessThan(Date().timeIntervalSince(start), 2.0 + 0.5)
        XCTAssertEqual(view.string, "にほんごkanji")
        XCTAssertFalse(view.hasMarkedText())
        XCTAssertFalse(session.isProcessingAsync)
        runMainLoop(for: 2.5)
        XCTAssertEqual(view.string, "にほんごkanji")
        XCTAssertTrue(handle(0x75))
        XCTAssertEqual(view.string, "にほんごkanjiう")
    }

    func testCompositionFlushPreservesQueuedRomajiWithinShortDeadline() throws {
        try assertBarrierPreservesQueuedRomaji {
            self.session.flush(hasPreedit: { !self.preedit.text.isEmpty }, apply: self.apply, recover: self.recover)
        }
    }

    func testDeactivationWithoutClientPreservesQueuedRomajiWithinShortDeadline() throws {
        try assertBarrierPreservesQueuedRomaji { self.session.flushPendingComposition() }
    }

    func testCandidateClickPreservesQueuedRomajiWithinShortDeadline() throws {
        try assertBarrierPreservesQueuedRomaji {
            self.session.selectCandidate(
                pageIndex: 0, hasPreedit: { !self.preedit.text.isEmpty }, apply: self.apply, recover: self.recover)
        }
    }

    func testFlushDrainsReadyQueueAndCommitsConvertedInputWithinShortDeadline() throws {
        try start(delay: 1, delayOnce: true)
        compose()
        XCTAssertTrue(handle(0xff0d))
        for key in "kanji".unicodeScalars { XCTAssertTrue(handle(Int(key.value))) }
        let start = Date()
        session.flush(hasPreedit: { !self.preedit.text.isEmpty }, apply: apply, recover: recover)
        XCTAssertLessThan(Date().timeIntervalSince(start), 2.0 + 0.5)
        XCTAssertEqual(view.string, "にほんごかんじ")
        XCTAssertEqual(commits, ["にほんご", "かんじ"])
        XCTAssertEqual(recoveries, 0)
        XCTAssertFalse(view.hasMarkedText())
    }

    func testCompositionFlushFinishesEnterOnlyOnce() throws {
        try start(delay: 4)
        compose()
        XCTAssertTrue(handle(0xff0d))
        session.flush(hasPreedit: { !self.preedit.text.isEmpty }, apply: apply, recover: recover)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        runMainLoop(for: 2.5)
        session.flush(hasPreedit: { !self.preedit.text.isEmpty }, apply: apply, recover: recover)
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertFalse(view.hasMarkedText())
    }

    func testDeactivationWithoutNewClientSettlesOriginalClient() throws {
        try start(delay: 4)
        compose()
        XCTAssertTrue(handle(0xff0d))
        session.finishPending(until: EngineClient.synchronousDeadline())
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertFalse(view.hasMarkedText())
        runMainLoop(for: 2.5)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
    }

    func testCandidateClickSettlesEnterBeforeSelection() throws {
        try start(delay: 4)
        compose()
        XCTAssertTrue(handle(0xff0d))
        session.selectCandidate(
            pageIndex: 0, hasPreedit: { !self.preedit.text.isEmpty }, apply: apply, recover: recover)
        XCTAssertEqual(commits, ["にほんご"])
        runMainLoop(for: 2.5)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
    }

    func testSpaceTimeoutPreservesVisiblePreeditAndConsumesKey() throws {
        try start(delay: 3, keys: [0x20], initializeConversion: true)
        compose()
        let start = Date()
        let consumed = handle(0x20)
        XCTAssertLessThan(Date().timeIntervalSince(start), 2.0 + 0.5)
        XCTAssertTrue(consumed)
        passThrough(0x20, consumed: consumed)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertFalse(view.hasMarkedText())
    }

    func testOrdinaryKeyTimeoutPreservesVisiblePreeditAndConsumesKey() throws {
        try start(delay: 3, keys: [0x61])
        compose()
        let consumed = handle(0x61)
        XCTAssertTrue(consumed)
        passThrough(0x61, consumed: consumed)
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
    }

    func testEmptyPreeditTimeoutStillPassesKeyThrough() throws {
        try start(delay: 3, keys: [0x61])
        let consumed = handle(0x61)
        XCTAssertFalse(consumed)
        passThrough(0x61, consumed: consumed)
        XCTAssertEqual(view.string, "a")
        XCTAssertEqual(commits, [])
    }

    func testEmptyReturnUsesShortSynchronousPathAndPassesThrough() throws {
        try start(delay: 4)
        let start = Date()
        let consumed = handle(0xff0d)
        XCTAssertLessThan(Date().timeIntervalSince(start), 2.0 + 0.5)
        XCTAssertFalse(consumed)
        passThrough(0xff0d, consumed: consumed)
        XCTAssertEqual(view.string, "\n")
        XCTAssertEqual(commits, [])
    }

    func testDelayedEnterCommitsRealConversionCandidate() throws {
        try start(delay: 4, initializeConversion: true)
        compose()
        // Prepare a real conversion outside the timing measurement; no fake candidates or responses.
        let data = try XCTUnwrap(client.sendRequestSync(method: "process_key", params: ["keysym": 0x20], timeout: 15))
        let result = try makeProtocolDecoder().decode(KeyResult.self, from: data)
        apply(result.actions)
        let selected = try XCTUnwrap(
            result.actions.compactMap { action -> String? in
                if case .showCandidates(let candidates, let cursor, _, _) = action {
                    return candidates[cursor ?? 0].text
                }
                return nil
            }.first)
        XCTAssertEqual(preedit.text, selected)
        let start = Date()
        XCTAssertTrue(handle(0xff0d))
        XCTAssertLessThan(Date().timeIntervalSince(start), 0.5)
        XCTAssertTrue(view.hasMarkedText())
        runMainLoop(for: 4.5)
        XCTAssertEqual(view.string, selected)
        XCTAssertEqual(commits, [selected])
        XCTAssertEqual(recoveries, 0, "The real response, not timeout recovery, must commit")
        XCTAssertFalse(view.hasMarkedText())
    }

    func testKanaSwitchTimeoutStillConsumesKeyWithoutPreedit() throws {
        try start(delay: 3, keys: [Int(KeyCodeMap.superRKeysym)])
        // The kana key is always consumed so macOS never processes keyCode 104.
        XCTAssertTrue(session.handleKanaSwitch(apply: apply, recover: recover))
        XCTAssertEqual(view.string, "")
        XCTAssertEqual(commits, [])
    }

    func testKanaSwitchTimeoutPreservesPreeditAndConsumesKey() throws {
        try start(delay: 3, keys: [Int(KeyCodeMap.superRKeysym)])
        compose()
        XCTAssertTrue(session.handleKanaSwitch(apply: apply, recover: recover))
        XCTAssertEqual(view.string, "にほんご")
        XCTAssertEqual(commits, ["にほんご"])
        XCTAssertFalse(view.hasMarkedText())
    }

    func testSuccessfulKanaSwitchStillConsumesAlreadyHiraganaKey() throws {
        try start(delay: 0)
        XCTAssertTrue(session.handleKanaSwitch(apply: apply, recover: recover))
        XCTAssertTrue(session.handleKanaSwitch(apply: apply, recover: recover))
        XCTAssertEqual(view.string, "")
        XCTAssertEqual(commits, [])
    }

}
