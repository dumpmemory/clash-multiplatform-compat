val releaseTag = System.getenv("TAG")?.trim()?.takeIf(String::isNotEmpty) ?: "latest"

subprojects {
    group = "com.github.kr328.clash.compat"
    version = releaseTag

    plugins.withId("java") {
        configure<JavaPluginExtension> {
            sourceCompatibility = JavaVersion.VERSION_16
            targetCompatibility = JavaVersion.VERSION_16
        }
    }

    plugins.withId("maven-publish") {
        extensions.configure<PublishingExtension> {
            publications {
                withType(MavenPublication::class) {
                    version = project.version.toString()
                    group = project.group.toString()

                    pom {
                        name.set("Clash Multiplatform Compat")
                        description.set("Clash multiplatform compat helpers")
                        url.set("https://github.com/Kr328/clash-multiplatform-compat")
                        licenses {
                            license {
                                name.set("MIT License")
                                url.set("https://github.com/Kr328/clash-multiplatform-compat/blob/main/LICENSE")
                            }
                        }
                        developers {
                            developer {
                                name.set("Kr328")
                            }
                        }
                        scm {
                            scm {
                                connection.set("scm:git:https://github.com/Kr328/clash-multiplatform-compat.git")
                                url.set("https://github.com/Kr328/clash-multiplatform-compat")
                            }
                        }
                    }
                }
                repositories {
                    mavenLocal()
                    maven {
                        val releaseUrl = if (releaseTag == "latest") {
                            "https://maven.kr328.app/snapshots"
                        } else {
                            "https://maven.kr328.app/releases"
                        }

                        name = "kr328app"
                        url = uri(releaseUrl)
                        credentials(PasswordCredentials::class)
                    }
                }
            }
        }
    }
}

task("clean", type = Delete::class) {
    group = "build"

    delete(buildDir)
}
