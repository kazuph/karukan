import Foundation
import XCTest

@testable import KarukanIME

/// The proxy only delays delivery; all responses come from the real Rust server.
final class EngineTimeoutTests: XCTestCase {
    private var process: EngineProcess!
    private var client: EngineClient!
    private var temporaryDirectory: URL!

    override func setUpWithError() throws {
        let binary = try XCTUnwrap(TransportTests.serverBinaryPath(), "Build the real server first")
        temporaryDirectory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at: temporaryDirectory, withIntermediateDirectories: true)
        let proxy = temporaryDirectory.appendingPathComponent("delay-server")
        // Four seconds exceeds both the original and revised synchronous deadlines.
        let script = """
            #!/usr/bin/python3
            import json, os, subprocess, sys, time
            env = dict(os.environ, HOME=\(String(reflecting: temporaryDirectory.path)))
            server = subprocess.Popen([\(String(reflecting: binary))], stdin=subprocess.PIPE, env=env)
            for line in sys.stdin:
                request = json.loads(line)
                if request['method'] in ('process_key', 'commit', 'select_candidate'):
                    time.sleep(4)
                server.stdin.write(line.encode())
                server.stdin.flush()
            server.stdin.close()
            server.wait()
            """
        try script.write(to: proxy, atomically: true, encoding: .utf8)
        try FileManager.default.setAttributes([.posixPermissions: 0o700], ofItemAtPath: proxy.path)
        process = EngineProcess(serverPath: proxy.path)
        client = EngineClient(serverProcess: process, autoInit: false)
        process.start()
        client.startReaderLoop()
        // Measure delayed RPCs only after the real server and proxy are ready.
        _ = try XCTUnwrap(client.sendRequestSync(method: "status", params: [:], timeout: 5))
    }

    override func tearDownWithError() throws {
        // A status reply is ordered after an abandoned key and lets the real server drain.
        XCTAssertNotNil(client.sendRequestSync(method: "status", params: [:], timeout: 5))
        process.stop()
        try FileManager.default.removeItem(at: temporaryDirectory)
    }

    func testEmptyReturnUsesShortDeadline() {
        XCTAssertNil(client.processKeySync(EngineKeyEvent(keysym: 0xff0d, modifiers: KeyModifiers())))
    }

    func testEmptyKeypadEnterUsesShortDeadline() {
        XCTAssertNil(client.processKeySync(EngineKeyEvent(keysym: 0xff8d, modifiers: KeyModifiers())))
    }

    func testCommitUsesShortDeadline() {
        XCTAssertNil(client.commitSync())
    }

    func testOrdinaryKeysUseShortDeadline() {
        let start = Date()
        XCTAssertNil(client.processKeySync(EngineKeyEvent(keysym: 0x61, modifiers: KeyModifiers())))
        XCTAssertLessThan(Date().timeIntervalSince(start), 2.0 + 0.5)
    }
    func testCandidateSelectionUsesShortDeadline() {
        XCTAssertNil(client.selectCandidateSync(pageIndex: 0))
    }

    func testKanaSwitchUsesShortDeadline() {
        XCTAssertNil(client.processKeySync(EngineKeyEvent(keysym: KeyCodeMap.superRKeysym, modifiers: KeyModifiers())))
    }

}
