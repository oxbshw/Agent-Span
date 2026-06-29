# AgentSpan for JetBrains IDEs

An IntelliJ Platform plugin that connects your JetBrains IDE to an
[AgentSpan](https://agentspan.com) gateway — the same HTTP gateway AI agents use
to **read and search the web** — so you can run those lookups without leaving the
editor.

Works in IntelliJ IDEA, PyCharm, WebStorm, GoLand, CLion, Rider, and other
IntelliJ-based IDEs (since-build `233`, i.e. 2023.3+).

## What it does

- **AgentSpan: Search Web** — prompts for a query and runs a *federated* search
  across all configured channels (`POST /api/v1/search/federated`). Results are
  listed with title, URL, snippet, and the channels each hit came from.
- **AgentSpan: Read URL** — prompts for a URL and fetches the extracted,
  readable content of the page (`GET /api/v1/read`).
- **AgentSpan tool window** — a bottom tool window that renders results as
  clickable HTML; links open in your system browser.

Both actions live under **Tools → AgentSpan** and have default shortcuts:

| Action                 | Shortcut                     |
| ---------------------- | ---------------------------- |
| AgentSpan: Search Web  | `Ctrl+Alt+Shift+S`           |
| AgentSpan: Read URL    | `Ctrl+Alt+Shift+R`           |

Requests run on a background thread under a cancelable progress indicator;
failures (e.g. the gateway is down) surface as IDE notifications.

## Settings

Open **Settings → Tools → AgentSpan**:

- **Server URL** — base URL of your AgentSpan gateway. Default
  `http://localhost:8080`.
- **API key** — optional. When set, it is sent as the `X-API-Key` header on every
  request.

Settings are application-level and persist in `agentspan.xml` in your IDE config
directory.

## Gateway endpoints used

| Action / feature     | Endpoint                                                    |
| -------------------- | ---------------------------------------------------------- |
| Health check         | `GET /health`                                              |
| Read URL             | `GET /api/v1/read?url=<url>`                               |
| List channels        | `GET /api/v1/channels`                                     |
| Per-channel search   | `GET /api/v1/channels/{name}/search?q=<q>&limit=10`        |
| Federated search     | `POST /api/v1/search/federated` — body `{query, limit}`   |

> The plugin also includes a `health()` and per-channel `searchChannel()` call in
> `AgentSpanClient` for completeness; the bundled actions use federated search and
> read.

## Development

This is a standard [IntelliJ Platform Gradle plugin (v1.x)](https://plugins.jetbrains.com/docs/intellij/tools-gradle-intellij-plugin.html)
project using Kotlin.

```bash
# Launch a sandbox IDE with the plugin installed
./gradlew runIde

# Build a distributable plugin ZIP (build/distributions/*.zip)
./gradlew buildPlugin

# Run tests
./gradlew test

# Verify plugin.xml compatibility against the target IDE
./gradlew verifyPlugin
```

Key versions are in `gradle.properties`:

- `platformVersion` — the IntelliJ build the plugin compiles/runs against
  (default `2023.3.6`, type `IC` = IntelliJ IDEA Community).
- `pluginSinceBuild` / `pluginUntilBuild` — supported IDE build range.

### Project layout

```
build.gradle.kts                Gradle build (Kotlin JVM + org.jetbrains.intellij)
settings.gradle.kts
gradle.properties               plugin + platform versions
src/main/resources/META-INF/
  plugin.xml                    id, actions, tool window, settings page
src/main/kotlin/com/agentspan/
  AgentSpanClient.kt            java.net.http.HttpClient wrapper over the REST API
  AgentSpanModels.kt            response data classes + AgentSpanException
  actions/
    SearchWebAction.kt          Tools → AgentSpan: Search Web
    ReadUrlAction.kt            Tools → AgentSpan: Read URL
    AgentSpanActionSupport.kt   background-task + notification + tool-window helpers
  settings/
    AgentSpanSettings.kt        PersistentStateComponent (serverUrl, apiKey)
    AgentSpanConfigurable.kt    Settings → Tools → AgentSpan UI
  toolwindow/
    AgentSpanToolWindowFactory.kt
    AgentSpanToolWindowPanel.kt HTML result rendering
```

## License

See the repository root.
