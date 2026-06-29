package com.agentspan.actions

import com.agentspan.AgentSpanClient
import com.agentspan.ReadResult
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.InputValidatorEx
import com.intellij.openapi.ui.Messages

/**
 * "AgentSpan: Read URL" action.
 *
 * Prompts for a URL, fetches readable content through the gateway on a
 * background thread, and renders it in the AgentSpan tool window.
 */
class ReadUrlAction : AnAction() {

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        val url = Messages.showInputDialog(
            project,
            "Enter a URL to read:",
            "AgentSpan: Read URL",
            Messages.getQuestionIcon(),
            "https://",
            UrlValidator,
        )?.trim().orEmpty()

        if (url.isEmpty() || url == "https://") return

        val client = AgentSpanClient()

        AgentSpanActionSupport.runInBackground(
            project = project,
            title = "AgentSpan: reading $url",
            work = { client.read(url) },
            onSuccess = { result: ReadResult ->
                AgentSpanActionSupport.withToolWindow(project) { panel ->
                    panel.showReadResult(result)
                }
            },
        )
    }

    /** Light client-side check; the gateway does the real fetching/validation. */
    private object UrlValidator : InputValidatorEx {
        override fun getErrorText(inputString: String?): String? {
            val value = inputString?.trim().orEmpty()
            if (value.isEmpty()) return "URL is required"
            if (!value.startsWith("http://") && !value.startsWith("https://")) {
                return "URL must start with http:// or https://"
            }
            return null
        }

        override fun checkInput(inputString: String?): Boolean = getErrorText(inputString) == null

        override fun canClose(inputString: String?): Boolean = checkInput(inputString)
    }
}
