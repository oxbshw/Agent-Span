import Foundation
#if canImport(FoundationNetworking)
import FoundationNetworking
#endif

/// Errors returned by the AgentSpan SDK.
public enum AgentSpanError: Error, Equatable {
    case authentication(String)
    case rateLimited(retryAfter: Int?)
    case api(status: Int, message: String)
    case channel(String)
    case transport(String)
}

/// Async client for the AgentSpan gateway.
///
/// Thin wrapper over the AgentSpan REST API (see docs/api-reference.md).
public struct AgentSpanClient {
    private let baseURL: String
    private let apiKey: String?
    private let session: URLSession

    public init(baseURL: String = "http://localhost:8080", apiKey: String? = nil, session: URLSession = .shared) {
        self.baseURL = baseURL.hasSuffix("/") ? String(baseURL.dropLast()) : baseURL
        self.apiKey = apiKey
        self.session = session
    }

    /// Read a URL via the best matching channel.
    public func read(_ url: String, forceRefresh: Bool = false) async throws -> [String: Any] {
        let data = try await request("GET", "/api/v1/read", query: [
            URLQueryItem(name: "url", value: url),
            URLQueryItem(name: "force_refresh", value: forceRefresh ? "true" : "false"),
        ])
        if let err = data["error"] as? String { throw AgentSpanError.channel(err) }
        return data["content"] as? [String: Any] ?? [:]
    }

    /// Search a platform via a named channel.
    public func search(_ channel: String, _ query: String, limit: Int = 10) async throws -> [[String: Any]] {
        let data = try await request("GET", "/api/v1/channels/\(channel)/search", query: [
            URLQueryItem(name: "q", value: query),
            URLQueryItem(name: "limit", value: String(limit)),
        ])
        if let err = data["error"] as? String { throw AgentSpanError.channel(err) }
        return data["results"] as? [[String: Any]] ?? []
    }

    /// List available channels.
    public func listChannels() async throws -> [[String: Any]] {
        let data = try await request("GET", "/api/v1/channels")
        return data["channels"] as? [[String: Any]] ?? []
    }

    /// Run health diagnostics across all channels.
    public func doctor() async throws -> [String: Any] {
        try await request("GET", "/api/v1/doctor")
    }

    /// Fetch the non-secret configuration view.
    public func getConfig() async throws -> [String: Any] {
        try await request("GET", "/api/v1/config")
    }

    /// Read many URLs in parallel (server-side batch).
    public func batchRead(_ urls: [String], forceRefresh: Bool = false) async throws -> [[String: Any]] {
        let data = try await request("POST", "/api/v1/batch/read",
                                     body: ["urls": urls, "force_refresh": forceRefresh])
        return data["results"] as? [[String: Any]] ?? []
    }

    /// Run many queries against one channel in parallel.
    public func batchSearch(_ channel: String, _ queries: [String], limit: Int = 10) async throws -> [[String: Any]] {
        let data = try await request("POST", "/api/v1/batch/search",
                                     body: ["channel": channel, "queries": queries, "limit": limit])
        return data["results"] as? [[String: Any]] ?? []
    }

    /// Return true when the server's /health endpoint is OK.
    public func health() async -> Bool {
        guard let url = URL(string: baseURL + "/health") else { return false }
        var req = URLRequest(url: url)
        if let key = apiKey { req.setValue(key, forHTTPHeaderField: "X-API-Key") }
        guard let (_, resp) = try? await session.data(for: req),
              let http = resp as? HTTPURLResponse else { return false }
        return http.statusCode == 200
    }

    // MARK: - Internals

    private func request(_ method: String, _ path: String,
                         query: [URLQueryItem]? = nil,
                         body: [String: Any]? = nil) async throws -> [String: Any] {
        var components = URLComponents(string: baseURL + path)!
        if let query { components.queryItems = query }
        guard let url = components.url else { throw AgentSpanError.transport("invalid URL") }

        var req = URLRequest(url: url)
        req.httpMethod = method
        req.setValue("application/json", forHTTPHeaderField: "Accept")
        if let key = apiKey { req.setValue(key, forHTTPHeaderField: "X-API-Key") }
        if let body {
            req.setValue("application/json", forHTTPHeaderField: "Content-Type")
            req.httpBody = try JSONSerialization.data(withJSONObject: body)
        }

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await session.data(for: req)
        } catch {
            throw AgentSpanError.transport(error.localizedDescription)
        }
        guard let http = response as? HTTPURLResponse else {
            throw AgentSpanError.transport("no HTTP response")
        }
        let json = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any] ?? [:]
        if http.statusCode >= 400 {
            throw classify(status: http.statusCode, json: json, response: http)
        }
        return json
    }

    private func classify(status: Int, json: [String: Any], response: HTTPURLResponse) -> AgentSpanError {
        let message = json["error"] as? String ?? "HTTP \(status)"
        switch status {
        case 401: return .authentication(message)
        case 429:
            let retry = (response.value(forHTTPHeaderField: "Retry-After")).flatMap { Int($0) }
            return .rateLimited(retryAfter: retry)
        default: return .api(status: status, message: message)
        }
    }
}
