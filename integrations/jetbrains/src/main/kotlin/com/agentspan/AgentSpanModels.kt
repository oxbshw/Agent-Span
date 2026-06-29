package com.agentspan

/**
 * Plain data carriers for the AgentSpan REST responses.
 *
 * These intentionally mirror the gateway JSON shapes loosely; the client tolerates
 * missing fields rather than failing hard, so optional values default to empty.
 */

/** Result of `GET /api/v1/read`. */
data class ReadResult(
    val title: String,
    val body: String,
    val url: String,
)

/** Entry from `GET /api/v1/channels`. */
data class Channel(
    val name: String,
    val description: String,
    val tier: String,
)

/**
 * A single search hit. [channels] is populated by federated search and empty for
 * single-channel search.
 */
data class SearchHit(
    val title: String,
    val url: String,
    val snippet: String,
    val channels: List<String> = emptyList(),
)

/** Raised when the gateway is unreachable or returns a non-success status. */
class AgentSpanException(message: String, cause: Throwable? = null) : Exception(message, cause)
