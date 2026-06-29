package com.agentspan.settings

import com.intellij.openapi.options.Configurable
import com.intellij.openapi.ui.DialogPanel
import com.intellij.ui.components.JBPasswordField
import com.intellij.ui.components.JBTextField
import com.intellij.ui.dsl.builder.AlignX
import com.intellij.ui.dsl.builder.bindText
import com.intellij.ui.dsl.builder.panel
import javax.swing.JComponent

/**
 * Settings page under *Settings -> Tools -> AgentSpan*.
 *
 * Exposes the gateway base URL and the optional API key. Uses the Kotlin UI DSL
 * so the apply/reset/modified plumbing is handled by the [DialogPanel] bindings.
 */
class AgentSpanConfigurable : Configurable {

    private val settings = AgentSpanSettings.getInstance()

    private val serverUrlField = JBTextField()
    private val apiKeyField = JBPasswordField()

    private var rootPanel: DialogPanel? = null

    override fun getDisplayName(): String = "AgentSpan"

    override fun createComponent(): JComponent {
        val ui = panel {
            group("Gateway") {
                row("Server URL:") {
                    cell(serverUrlField)
                        .align(AlignX.FILL)
                        .comment("Base URL of the AgentSpan gateway, e.g. http://localhost:8080")
                        .bindText(settings::serverUrl)
                }
                row("API key:") {
                    cell(apiKeyField)
                        .align(AlignX.FILL)
                        .comment("Optional. Sent as the X-API-Key header when set.")
                        .bindText(
                            getter = { settings.apiKey },
                            setter = { settings.apiKey = it },
                        )
                }
            }
        }
        rootPanel = ui
        return ui
    }

    override fun isModified(): Boolean = rootPanel?.isModified() ?: false

    override fun apply() {
        rootPanel?.apply()
    }

    override fun reset() {
        rootPanel?.reset()
    }

    override fun disposeUIResources() {
        rootPanel = null
    }
}
