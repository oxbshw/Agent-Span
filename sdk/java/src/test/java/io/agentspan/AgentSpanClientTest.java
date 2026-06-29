package io.agentspan;

import static org.junit.jupiter.api.Assertions.*;

import java.util.List;
import okhttp3.mockwebserver.MockResponse;
import okhttp3.mockwebserver.MockWebServer;
import org.junit.jupiter.api.AfterEach;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;

class AgentSpanClientTest {
    private MockWebServer server;
    private AgentSpanClient client;

    @BeforeEach
    void setUp() throws Exception {
        server = new MockWebServer();
        server.start();
        client = new AgentSpanClient(server.url("/").toString().replaceAll("/$", ""), "k");
    }

    @AfterEach
    void tearDown() throws Exception {
        server.shutdown();
    }

    @Test
    void readReturnsContent() {
        server.enqueue(new MockResponse()
                .setBody("{\"channel\":\"web\",\"content\":{\"body\":\"hi\",\"title\":\"T\"}}"));
        assertEquals("hi", client.read("https://x", false).get("body").asText());
    }

    @Test
    void readChannelError() {
        server.enqueue(new MockResponse().setBody("{\"error\":\"no channel\"}"));
        assertThrows(AgentSpanClient.ChannelException.class, () -> client.read("ftp://x", false));
    }

    @Test
    void searchMapsResults() {
        server.enqueue(new MockResponse()
                .setBody("{\"results\":[{\"title\":\"Rust\",\"url\":\"https://r\",\"snippet\":\"s\"}]}"));
        assertEquals("Rust", client.search("hackernews", "rust", 7).get(0).get("title").asText());
    }

    @Test
    void authError() {
        server.enqueue(new MockResponse().setResponseCode(401).setBody("{\"error\":\"bad\"}"));
        assertThrows(AgentSpanClient.AuthenticationException.class, () -> client.listChannels());
    }

    @Test
    void rateLimitCarriesRetryAfter() {
        server.enqueue(new MockResponse()
                .setResponseCode(429)
                .setHeader("Retry-After", "12")
                .setBody("{\"error\":\"slow\"}"));
        var ex = assertThrows(
                AgentSpanClient.RateLimitException.class, () -> client.read("https://x", false));
        assertEquals(12, ex.retryAfter);
    }

    @Test
    void batchRead() throws Exception {
        server.enqueue(new MockResponse()
                .setBody("{\"count\":2,\"results\":[{\"ok\":true},{\"ok\":false}]}"));
        assertEquals(2, client.batchRead(List.of("a", "b"), false).size());
    }

    @Test
    void health() {
        server.enqueue(new MockResponse().setResponseCode(200));
        assertTrue(client.health());
    }
}
