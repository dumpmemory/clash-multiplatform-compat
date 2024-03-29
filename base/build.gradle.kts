import java.util.*

plugins {
    java
    `maven-publish`
}

dependencies {
    if (System.getenv("COMPAT_LIBRARY_PATH") == null) {
        testImplementation(project(":jni", configuration = "windows-amd64-debug"))
        testImplementation(project(":jni", configuration = "linux-amd64-debug"))
    }

    implementation("org.jetbrains:annotations:24.0.1")
    testImplementation("org.junit.jupiter:junit-jupiter:5.9.3")
}

java {
    withSourcesJar()
}

tasks.test {
    useJUnitPlatform()

    jvmArgs = Objects.requireNonNull(jvmArgs) + listOf(
        "--add-opens", "java.base/sun.nio.ch=ALL-UNNAMED",
        "--add-opens", "java.base/java.io=ALL-UNNAMED",
        "--add-opens", "java.desktop/java.awt=ALL-UNNAMED",
        "--add-opens", "java.desktop/sun.awt.windows=ALL-UNNAMED",
        "--add-opens", "java.desktop/sun.awt.X11=ALL-UNNAMED",
    )
}

publishing {
    publications {
        create("compat", MavenPublication::class) {
            artifactId = "compat"

            from(components["java"])
        }
    }
}
