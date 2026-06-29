package com.agentspan.actions

import com.agentspan.AgentSpanClient
import com.agentspan.SearchHit
import com.intellij.openapi.actionSystem.AnAction
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.ui.Messages

/**
 * "AgentSpan: Search Web" action.
 *
 * Prompts for a query, runs a federated search through the gateway on a
 * background thread, and renders the hits in the AgentSpan tool window.
 */
class SearchWebAction : AnAction() {

    override fun actionPerformed(e: AnActionEvent) {
        val project = e.project ?: return

        val query = Messages.showInputDialog(
            project,
            "Enter a search query:",
            "AgentSpan: Search Web",
            Messages.getQuestionIcon(),
        )?.trim().orEmpty()

        if (query.isEmpty()) return

        val client = AgentSpanClient()

        AgentSpanActionSupport.runInBackground(
            project = project,
            title = "AgentSpan: searching for \"$query\"",
            work = { client.searchFederated(query, limit = 10) },
            onSuccess = { hits: List<SearchHit> ->
                AgentSpanActionSupport.withToolWindow(project) { panel ->
                    panel.showSearchResults(query, hits)
                }
            },
        )
    }
}
