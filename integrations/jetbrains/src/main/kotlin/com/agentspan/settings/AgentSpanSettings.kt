package com.agentspan.settings

import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage

/**
 * Application-level persistent settings for the AgentSpan plugin.
 *
 * Stored in `agentspan.xml` in the IDE config directory. The API key is kept
 * here as plain state for simplicity; teams that need secret storage can move it
 * to the platform [com.intellij.credentialStore.PasswordSafe] later.
 */
@State(
    name = "com.agentspan.settings.AgentSpanSettings",
    storages = [Storage("agentspan.xml")],
)
class AgentSpanSettings : PersistentStateComponent<AgentSpanSettings.State> {

    data class State(
        var serverUrl: String = DEFAULT_SERVER_URL,
        var apiKey: String = "",
    )

    private var state = State()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    /** Base URL of the AgentSpan gateway, normalized without a trailing slash. */
    var serverUrl: String
        get() = state.serverUrl.ifBlank { DEFAULT_SERVER_URL }.trimEnd('/')
        set(value) {
            state.serverUrl = value.trim()
        }

    /** Optional API key, sent as the `X-API-Key` header when non-blank. */
    var apiKey: String
        get() = state.apiKey
        set(value) {
            state.apiKey = value.trim()
        }

    companion object {
        const val DEFAULT_SERVER_URL = "http://localhost:8080"

        @JvmStatic
        fun getInstance(): AgentSpanSettings =
            ApplicationManager.getApplication().getService(AgentSpanSettings::class.java)
    }
}
