package com.agentspan.toolwindow

import com.agentspan.ReadResult
import com.agentspan.SearchHit
import com.intellij.openapi.project.Project
import com.intellij.ui.components.JBScrollPane
import com.intellij.util.ui.JBUI
import com.intellij.util.ui.UIUtil
import java.awt.BorderLayout
import javax.swing.JEditorPane
import javax.swing.JPanel
import javax.swing.event.HyperlinkEvent

/**
 * The content panel for the AgentSpan tool window.
 *
 * Renders results as lightweight HTML inside a [JEditorPane]. Links are
 * clickable and open in the system browser. A singleton-per-project accessor
 * lets actions push results in without holding a direct reference.
 */
class AgentSpanToolWindowPanel(@Suppress("unused") private val project: Project) : JPanel(BorderLayout()) {

    private val editorPane = JEditorPane("text/html", "").apply {
        isEditable = false
        background = UIUtil.getPanelBackground()
        border = JBUI.Borders.empty(8)
        addHyperlinkListener { event ->
            if (event.eventType == HyperlinkEvent.EventType.ACTIVATED) {
                event.url?.let { com.intellij.ide.BrowserUtil.browse(it) }
            }
        }
    }

    init {
        add(JBScrollPane(editorPane), BorderLayout.CENTER)
        showMessage("Run <b>Tools &rarr; AgentSpan</b> to search the web or read a URL.")
        register(project, this)
    }

    fun showMessage(html: String) {
        editorPane.text = wrap(html)
        editorPane.caretPosition = 0
    }

    fun showReadResult(result: ReadResult) {
        val sb = StringBuilder()
        sb.append("<h2>").append(escape(result.title)).append("</h2>")
        sb.append("<p><a href=\"").append(escapeAttr(result.url)).append("\">")
            .append(escape(result.url)).append("</a></p>")
        sb.append("<hr/>")
        sb.append("<div>").append(bodyToHtml(result.body)).append("</div>")
        showMessage(sb.toString())
    }

    fun showSearchResults(query: String, hits: List<SearchHit>) {
        val sb = StringBuilder()
        sb.append("<h2>Results for &ldquo;").append(escape(query)).append("&rdquo;</h2>")
        if (hits.isEmpty()) {
            sb.append("<p><i>No results.</i></p>")
        } else {
            sb.append("<p><small>").append(hits.size).append(" result(s)</small></p>")
            hits.forEachIndexed { index, hit ->
                sb.append("<div style=\"margin-bottom:12px;\">")
                sb.append("<b>").append(index + 1).append(". </b>")
                sb.append("<a href=\"").append(escapeAttr(hit.url)).append("\">")
                    .append(escape(hit.title.ifBlank { hit.url })).append("</a>")
                if (hit.channels.isNotEmpty()) {
                    sb.append(" <small>[").append(escape(hit.channels.joinToString(", ")))
                        .append("]</small>")
                }
                sb.append("<br/><small>").append(escape(hit.url)).append("</small>")
                if (hit.snippet.isNotBlank()) {
                    sb.append("<br/>").append(escape(hit.snippet))
                }
                sb.append("</div>")
            }
        }
        showMessage(sb.toString())
    }

    private fun wrap(inner: String): String {
        val fg = colorHex(UIUtil.getLabelForeground())
        val font = UIUtil.getLabelFont()
        return """
            <html><head><style>
              body { font-family: ${font.family}; font-size: ${font.size}pt; color: $fg; }
              a { color: ${colorHex(JBUI.CurrentTheme.Link.Foreground.ENABLED)}; }
              hr { border: 0; border-top: 1px solid #888; }
            </style></head><body>$inner</body></html>
        """.trimIndent()
    }

    private fun bodyToHtml(body: String): String =
        escape(body).replace("\n\n", "<br/><br/>").replace("\n", "<br/>")

    private fun escape(text: String): String = buildString(text.length) {
        for (c in text) {
            when (c) {
                '&' -> append("&amp;")
                '<' -> append("&lt;")
                '>' -> append("&gt;")
                '"' -> append("&quot;")
                '\'' -> append("&#39;")
                else -> append(c)
            }
        }
    }

    private fun escapeAttr(text: String): String = escape(text)

    private fun colorHex(color: java.awt.Color): String =
        String.format("#%02x%02x%02x", color.red, color.green, color.blue)

    companion object {
        private val panels = mutableMapOf<Project, AgentSpanToolWindowPanel>()

        private fun register(project: Project, panel: AgentSpanToolWindowPanel) {
            panels[project] = panel
        }

        /** Returns the live panel for the project, or null if the tool window was never opened. */
        fun getInstance(project: Project): AgentSpanToolWindowPanel? = panels[project]
    }
}
