package com.agentspan

import com.agentspan.settings.AgentSpanSettings
import org.json.JSONArray
import org.json.JSONObject
import java.net.URI
import java.net.URLEncoder
import java.net.http.HttpClient
import java.net.http.HttpRequest
import java.net.http.HttpResponse
import java.nio.charset.StandardCharsets
import java.time.Duration

/**
 * Thin HTTP client over the AgentSpan gateway REST API.
 *
 * Reads connection settings from [AgentSpanSettings] on each call, so changes in
 * the settings dialog take effect immediately. All methods are blocking and must
 * be invoked off the EDT (the actions run them on a background thread).
 */
class AgentSpanClient(
    private val settings: AgentSpanSettings = AgentSpanSettings.getInstance(),
) {

    private val httpClient: HttpClient = HttpClient.newBuilder()
        .connectTimeout(Duration.ofSeconds(10))
        .followRedirects(HttpClient.Redirect.NORMAL)
        .build()

    private val baseUrl: String get() = settings.serverUrl

    // --- Public API ---------------------------------------------------------

    /** `GET /health` -> true when the gateway reports `{"status":"ok"}`. */
    fun health(): Boolean {
        val body = get("/health")
        return runCatching { JSONObject(body).optString("status") == "ok" }
            .getOrDefault(false)
    }

    /** `GET /api/v1/read?url=<url>`. */
    fun read(url: String): ReadResult {
        val body = get("/api/v1/read?url=${encode(url)}")
        val content = JSONObject(body).optJSONObject("content") ?: JSONObject()
        return ReadResult(
            title = content.optString("title").ifBlank { url },
            body = content.optString("body"),
            url = url,
        )
    }

    /** `GET /api/v1/channels`. */
    fun channels(): List<Channel> {
        val body = get("/api/v1/channels")
        val array = JSONObject(body).optJSONArray("channels") ?: JSONArray()
        return array.mapObjects { obj ->
            Channel(
                name = obj.optString("name"),
                description = obj.optString("description"),
                tier = obj.optString("tier"),
            )
        }
    }

    /** `GET /api/v1/channels/{name}/search?q=<q>&limit=<limit>`. */
    fun searchChannel(channel: String, query: String, limit: Int = 10): List<SearchHit> {
        val path = "/api/v1/channels/${encode(channel)}/search?q=${encode(query)}&limit=$limit"
        val body = get(path)
        val array = JSONObject(body).optJSONArray("results") ?: JSONArray()
        return array.mapObjects { obj ->
            SearchHit(
                title = obj.optString("title"),
                url = obj.optString("url"),
                snippet = obj.optString("snippet"),
            )
        }
    }

    /** `POST /api/v1/search/federated` with body `{query, limit}`. */
    fun searchFederated(query: String, limit: Int = 10): List<SearchHit> {
        val payload = JSONObject()
            .put("query", query)
            .put("limit", limit)
            .toString()
        val body = post("/api/v1/search/federated", payload)
        val array = JSONObject(body).optJSONArray("results") ?: JSONArray()
        return array.mapObjects { obj ->
            SearchHit(
                title = obj.optString("title"),
                url = obj.optString("url"),
                snippet = obj.optString("snippet"),
                channels = obj.optJSONArray("channels")?.toStringList() ?: emptyList(),
            )
        }
    }

    // --- Transport ----------------------------------------------------------

    private fun get(path: String): String =
        send(baseRequest(path).GET().build())

    private fun post(path: String, jsonBody: String): String =
        send(
            baseRequest(path)
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(jsonBody, StandardCharsets.UTF_8))
                .build(),
        )

    private fun baseRequest(path: String): HttpRequest.Builder {
        val builder = HttpRequest.newBuilder()
            .uri(URI.create(baseUrl + path))
            .timeout(Duration.ofSeconds(30))
            .header("Accept", "application/json")
            .header("User-Agent", "AgentSpan-IntelliJ")

        val key = settings.apiKey
        if (key.isNotBlank()) {
            builder.header("X-API-Key", key)
        }
        return builder
    }

    private fun send(request: HttpRequest): String {
        val response: HttpResponse<String> = try {
            httpClient.send(request, HttpResponse.BodyHandlers.ofString(StandardCharsets.UTF_8))
        } catch (e: Exception) {
            throw AgentSpanException(
                "Could not reach AgentSpan gateway at $baseUrl. Is it running? (${e.message})",
                e,
            )
        }

        if (response.statusCode() !in 200..299) {
            throw AgentSpanException(
                "AgentSpan request failed: HTTP ${response.statusCode()} for ${request.uri()}",
            )
        }
        return response.body()
    }

    // --- Helpers ------------------------------------------------------------

    private fun encode(value: String): String =
        URLEncoder.encode(value, StandardCharsets.UTF_8)

    private inline fun <T> JSONArray.mapObjects(transform: (JSONObject) -> T): List<T> =
        (0 until length()).mapNotNull { i -> optJSONObject(i)?.let(transform) }

    private fun JSONArray.toStringList(): List<String> =
        (0 until length()).map { optString(it) }.filter { it.isNotBlank() }
}
