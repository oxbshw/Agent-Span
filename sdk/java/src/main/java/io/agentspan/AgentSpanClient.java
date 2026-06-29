package io.agentspan;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.net.URI;
import java.net.URLEncoder;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.nio.charset.StandardCharsets;
import java.util.List;

/**
 * Official Java client for the AgentSpan gateway.
 *
 * <p>Thin wrapper over the AgentSpan REST API (see docs/api-reference.md), built
 * on {@code java.net.http.HttpClient} + Jackson.
 */
public final class AgentSpanClient {
    private final String baseUrl;
    private final String apiKey;
    private final HttpClient http;
    private final ObjectMapper mapper = new ObjectMapper();

    public AgentSpanClient(String baseUrl) {
        this(baseUrl, null);
    }

    public AgentSpanClient(String baseUrl, String apiKey) {
        this.baseUrl = baseUrl.replaceAll("/+$", "");
        this.apiKey = apiKey;
        this.http = HttpClient.newHttpClient();
    }

    /** Base class for all SDK errors. */
    public static class AgentSpanException extends RuntimeException {
        public AgentSpanException(String message) {
            super(message);
        }
    }

    /** Thrown on HTTP 401. */
    public static final class AuthenticationException extends AgentSpanException {
        public AuthenticationException(String m) {
            super(m);
        }
    }

    /** Thrown on HTTP 429. */
    public static final class RateLimitException extends AgentSpanException {
        public final Integer retryAfter;

        public RateLimitException(String m, Integer retryAfter) {
            super(m);
            this.retryAfter = retryAfter;
        }
    }

    /** Thrown for other non-2xx responses. */
    public static final class ApiException extends AgentSpanException {
        public final int status;

        public ApiException(int status, String m) {
            super(m);
            this.status = status;
        }
    }

    /** Thrown when the server embeds {"error":...} in a 200 body. */
    public static final class ChannelException extends AgentSpanException {
        public ChannelException(String m) {
            super(m);
        }
    }

    private static String enc(String s) {
        return URLEncoder.encode(s, StandardCharsets.UTF_8);
    }

    private HttpRequest.Builder builder(String path) {
        HttpRequest.Builder b = HttpRequest.newBuilder(URI.create(baseUrl + path))
                .header("Accept", "application/json");
        if (apiKey != null) {
            b.header("X-API-Key", apiKey);
        }
        return b;
    }

    private JsonNode send(HttpRequest req) {
        try {
            HttpResponse<String> res = http.send(req, HttpResponse.BodyHandlers.ofString());
            int status = res.statusCode();
            JsonNode body = res.body() == null || res.body().isEmpty()
                    ? mapper.createObjectNode()
                    : mapper.readTree(res.body());
            if (status >= 400) {
                throw classify(status, body, res);
            }
            return body;
        } catch (AgentSpanException e) {
            throw e;
        } catch (Exception e) {
            throw new AgentSpanException("transport error: " + e.getMessage());
        }
    }

    private AgentSpanException classify(int status, JsonNode body, HttpResponse<String> res) {
        String message = body.has("error") ? body.get("error").asText() : "HTTP " + status;
        return switch (status) {
            case 401 -> new AuthenticationException(message);
            case 429 -> new RateLimitException(
                    message,
                    res.headers().firstValue("Retry-After").map(Integer::parseInt).orElse(null));
            default -> new ApiException(status, message);
        };
    }

    /** Read a URL via the best matching channel; returns the content node. */
    public JsonNode read(String url, boolean forceRefresh) {
        JsonNode data = send(builder("/api/v1/read?url=" + enc(url) + "&force_refresh=" + forceRefresh)
                .GET().build());
        if (data.has("error")) {
            throw new ChannelException(data.get("error").asText());
        }
        return data.get("content");
    }

    /** Search a platform via a named channel; returns the results array. */
    public JsonNode search(String channel, String query, int limit) {
        JsonNode data = send(builder("/api/v1/channels/" + enc(channel) + "/search?q=" + enc(query)
                + "&limit=" + limit).GET().build());
        if (data.has("error")) {
            throw new ChannelException(data.get("error").asText());
        }
        return data.get("results");
    }

    /** List available channels. */
    public JsonNode listChannels() {
        return send(builder("/api/v1/channels").GET().build()).get("channels");
    }

    /** Run health diagnostics across all channels. */
    public JsonNode doctor() {
        return send(builder("/api/v1/doctor").GET().build());
    }

    /** Fetch the non-secret configuration view. */
    public JsonNode getConfig() {
        return send(builder("/api/v1/config").GET().build());
    }

    /** Read many URLs in parallel (server-side batch). */
    public JsonNode batchRead(List<String> urls, boolean forceRefresh) throws Exception {
        String body = mapper.writeValueAsString(
                java.util.Map.of("urls", urls, "force_refresh", forceRefresh));
        return send(builder("/api/v1/batch/read")
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(body)).build()).get("results");
    }

    /** Run many queries against one channel in parallel. */
    public JsonNode batchSearch(String channel, List<String> queries, int limit) throws Exception {
        String body = mapper.writeValueAsString(
                java.util.Map.of("channel", channel, "queries", queries, "limit", limit));
        return send(builder("/api/v1/batch/search")
                .header("Content-Type", "application/json")
                .POST(HttpRequest.BodyPublishers.ofString(body)).build()).get("results");
    }

    /** Return true when the server's /health endpoint is OK. */
    public boolean health() {
        try {
            HttpResponse<String> res = http.send(
                    builder("/health").GET().build(), HttpResponse.BodyHandlers.ofString());
            return res.statusCode() == 200;
        } catch (Exception e) {
            return false;
        }
    }
}
