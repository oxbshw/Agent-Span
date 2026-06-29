package com.agentspan.actions

import com.agentspan.toolwindow.AgentSpanToolWindowPanel
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.progress.ProgressIndicator
import com.intellij.openapi.progress.ProgressManager
import com.intellij.openapi.progress.Task
import com.intellij.openapi.project.Project
import com.intellij.openapi.wm.ToolWindowManager

/**
 * Shared helpers for AgentSpan actions: running gateway calls on a background
 * task, surfacing failures as notifications, and revealing results in the tool
 * window.
 */
internal object AgentSpanActionSupport {

    const val TOOL_WINDOW_ID = "AgentSpan"
    private const val NOTIFICATION_GROUP = "AgentSpan"

    /**
     * Runs [work] on a pooled background thread under a cancelable progress
     * indicator, then hands the result to [onSuccess] on the EDT. Exceptions are
     * reported as error notifications.
     */
    fun <T> runInBackground(
        project: Project,
        title: String,
        work: (ProgressIndicator) -> T,
        onSuccess: (T) -> Unit,
    ) {
        ProgressManager.getInstance().run(object : Task.Backgroundable(project, title, true) {
            private var result: T? = null
            private var failure: Throwable? = null

            override fun run(indicator: ProgressIndicator) {
                indicator.isIndeterminate = true
                try {
                    result = work(indicator)
                } catch (t: Throwable) {
                    failure = t
                }
            }

            override fun onFinished() {
                ApplicationManager.getApplication().invokeLater {
                    val error = failure
                    if (error != null) {
                        notifyError(project, error.message ?: "AgentSpan request failed.")
                    } else {
                        @Suppress("UNCHECKED_CAST")
                        onSuccess(result as T)
                    }
                }
            }
        })
    }

    /** Opens the AgentSpan tool window (creating it if needed) and runs [then]. */
    fun withToolWindow(project: Project, then: (AgentSpanToolWindowPanel) -> Unit) {
        val toolWindow = ToolWindowManager.getInstance(project).getToolWindow(TOOL_WINDOW_ID)
        if (toolWindow == null) {
            notifyError(project, "AgentSpan tool window is not available.")
            return
        }
        toolWindow.activate {
            val panel = AgentSpanToolWindowPanel.getInstance(project)
            if (panel != null) {
                then(panel)
            } else {
                notifyError(project, "AgentSpan tool window failed to initialize.")
            }
        }
    }

    fun notifyError(project: Project, message: String) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP)
            .createNotification(message, NotificationType.ERROR)
            .notify(project)
    }

    fun notifyInfo(project: Project, message: String) {
        NotificationGroupManager.getInstance()
            .getNotificationGroup(NOTIFICATION_GROUP)
            .createNotification(message, NotificationType.INFORMATION)
            .notify(project)
    }
}
