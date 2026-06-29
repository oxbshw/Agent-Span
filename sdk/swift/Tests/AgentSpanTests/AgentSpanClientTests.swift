import XCTest
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif
@testable import AgentSpan

/// URLProtocol that returns a canned response queued by the test (no network).
final class StubURLProtocol: URLProtocol {
    nonisolated(unsafe) static var responder: ((URLRequest) -> (Int, [String: String], Data))?

    override class func canInit(with request: URLRequest) -> Bool { true }
    override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }

    override func startLoading() {
        let (status, headers, body) = StubURLProtocol.responder?(request) ?? (200, [:], Data())
        let response = HTTPURLResponse(url: request.url!, statusCode: status, httpVersion: nil, headerFields: headers)!
        client?.urlProtocol(self, didReceive: response, cacheStoragePolicy: .notAllowed)
        client?.urlProtocol(self, didLoad: body)
        client?.urlProtocolDidFinishLoading(self)
    }

    override func stopLoading() {}
}

final class AgentSpanClientTests: XCTestCase {
    private func makeClient(_ responder: @escaping (URLRequest) -> (Int, [String: String], Data)) -> AgentSpanClient {
        StubURLProtocol.responder = responder
        let config = URLSessionConfiguration.ephemeral
        config.protocolClasses = [StubURLProtocol.self]
        return AgentSpanClient(baseURL: "http://test", apiKey: "k", session: URLSession(configuration: config))
    }

    private func json(_ status: Int, _ body: String, _ headers: [String: String] = [:]) -> (Int, [String: String], Data) {
        (status, headers, Data(body.utf8))
    }

    func testReadReturnsContent() async throws {
        let client = makeClient { _ in self.json(200, #"{"channel":"web","content":{"body":"hi"}}"#) }
        let content = try await client.read("https://x")
        XCTAssertEqual(content["body"] as? String, "hi")
    }

    func testReadChannelError() async {
        let client = makeClient { _ in self.json(200, #"{"error":"no channel"}"#) }
        do {
            _ = try await client.read("ftp://x")
            XCTFail("expected channel error")
        } catch {
            XCTAssertEqual(error as? AgentSpanError, .channel("no channel"))
        }
    }

    func testSearchMapsResults() async throws {
        let client = makeClient { _ in self.json(200, #"{"results":[{"title":"Rust"}]}"#) }
        let results = try await client.search("hackernews", "rust", limit: 7)
        XCTAssertEqual(results.first?["title"] as? String, "Rust")
    }

    func testAuthError() async {
        let client = makeClient { _ in self.json(401, #"{"error":"bad"}"#) }
        do {
            _ = try await client.listChannels()
            XCTFail("expected auth error")
        } catch {
            XCTAssertEqual(error as? AgentSpanError, .authentication("bad"))
        }
    }

    func testRateLimitCarriesRetryAfter() async {
        let client = makeClient { _ in self.json(429, #"{"error":"slow"}"#, ["Retry-After": "12"]) }
        do {
            _ = try await client.read("https://x")
            XCTFail("expected rate limit")
        } catch {
            XCTAssertEqual(error as? AgentSpanError, .rateLimited(retryAfter: 12))
        }
    }

    func testBatchRead() async throws {
        let client = makeClient { _ in self.json(200, #"{"count":2,"results":[{"ok":true},{"ok":false}]}"#) }
        let results = try await client.batchRead(["a", "b"])
        XCTAssertEqual(results.count, 2)
    }
}
