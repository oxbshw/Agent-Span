plugins {
    id("java")
    // Kotlin JVM
    kotlin("jvm") version "1.9.24"
    // IntelliJ Platform Gradle plugin (Gradle IntelliJ Plugin v1.x line).
    id("org.jetbrains.intellij") version "1.17.4"
}

group = providers.gradleProperty("pluginGroup").get()
version = providers.gradleProperty("pluginVersion").get()

repositories {
    mavenCentral()
}

dependencies {
    // JSON parsing. org.json keeps the client dependency-light; the IntelliJ
    // Platform already bundles a copy at runtime, so this is mostly for compile.
    implementation("org.json:json:20240303")

    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter:5.10.2")
}

// Configure the IntelliJ Platform Gradle plugin.
// See https://plugins.jetbrains.com/docs/intellij/tools-gradle-intellij-plugin.html
intellij {
    version.set(providers.gradleProperty("platformVersion"))
    type.set(providers.gradleProperty("platformType"))

    // No extra bundled plugins required for a Tools-menu + ToolWindow plugin.
    plugins.set(emptyList<String>())
}

kotlin {
    jvmToolchain(17)
}

tasks {
    withType<JavaCompile> {
        sourceCompatibility = "17"
        targetCompatibility = "17"
    }

    patchPluginXml {
        version.set(providers.gradleProperty("pluginVersion"))
        sinceBuild.set(providers.gradleProperty("pluginSinceBuild"))
        untilBuild.set(providers.gradleProperty("pluginUntilBuild"))
    }

    runIde {
        // Helpful when developing; the sandbox IDE picks up the plugin here.
        autoReloadPlugins.set(true)
    }

    test {
        useJUnitPlatform()
    }
}
