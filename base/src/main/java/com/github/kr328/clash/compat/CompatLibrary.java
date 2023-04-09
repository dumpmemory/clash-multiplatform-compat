package com.github.kr328.clash.compat;

import org.jetbrains.annotations.Nullable;

import java.io.IOException;
import java.io.InputStream;
import java.net.MalformedURLException;
import java.net.URL;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.FileTime;
import java.util.Comparator;
import java.util.Objects;
import java.util.stream.Stream;

public final class CompatLibrary {
    @Nullable
    private static Path overrideExtractPath = null;

    public static void setOverrideExtractPath(@Nullable final Path overrideExtractPath) {
        CompatLibrary.overrideExtractPath = overrideExtractPath;
    }

    public static void load() {
        Objects.requireNonNull(Loader.object);
    }

    private static final class Loader {
        public static final Object object = new Object();

        static {
            final String osName = System.getProperty("os.name").toLowerCase();

            final String extension;
            if (osName.contains("windows")) {
                extension = ".dll";
            } else if (osName.contains("linux")) {
                extension = ".so";
            } else {
                throw new UnsupportedOperationException("Unsupported os " + osName);
            }

            final String osArch = System.getProperty("os.arch").toLowerCase();

            final String arch;
            if (osArch.contains("amd64") || osArch.contains("x86_64")) {
                arch = "amd64";
            } else if (osArch.contains("x86") || osArch.contains("386") || osArch.contains("686")) {
                arch = "x86";
            } else {
                throw new LinkageError("Unsupported arch " + osArch);
            }

            final String fileName = "libcompat-" + arch + extension;

            final URL url;
            if (System.getenv("COMPAT_LIBRARY_PATH") != null) {
                try {
                    url = Path.of(System.getenv("COMPAT_LIBRARY_PATH")).toUri().toURL();
                } catch (final MalformedURLException e) {
                    throw new RuntimeException(e);
                }
            } else {
                url = Objects.requireNonNull(Loader.class.getResource("/com/github/kr328/clash/compat/" + fileName));
            }

            try {
                final Path extractPath;
                if (overrideExtractPath != null) {
                    extractPath = overrideExtractPath;

                    Files.createDirectories(extractPath);
                } else {
                    extractPath = Files.createTempDirectory("clash-multiplatform-compat-");

                    Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                        try (final Stream<Path> files = Files.walk(extractPath)) {
                            for (final Path file : files.sorted(Comparator.reverseOrder()).toList()) {
                                Files.delete(file);
                            }
                        } catch (final IOException e) {
                            // ignored
                        }
                    }));
                }

                if ("file".equals(url.getProtocol())) {
                    System.load(Path.of(url.toURI()).toAbsolutePath().toString());
                } else {
                    final Path libraryPath = extractPath.resolve(fileName);

                    FileTime libraryFsTime;
                    try {
                        libraryFsTime = Files.getLastModifiedTime(libraryPath);
                    } catch (final Exception e) {
                        libraryFsTime = FileTime.fromMillis(0);
                    }

                    FileTime jarFsTime;
                    try {
                        jarFsTime = Files.getLastModifiedTime(Path.of(CompatLibrary.class.getProtectionDomain().getCodeSource().getLocation().getPath()));
                    } catch (final Exception e) {
                        jarFsTime = FileTime.fromMillis(Long.MAX_VALUE);
                    }

                    if (jarFsTime.compareTo(libraryFsTime) > 0) {
                        try (final InputStream stream = url.openStream()) {
                            Files.copy(stream, libraryPath, StandardCopyOption.REPLACE_EXISTING);
                        }
                    }

                    System.load(libraryPath.toAbsolutePath().toString());
                }
            } catch (final Exception e) {
                throw new LinkageError("Load " + fileName, e);
            }
        }
    }
}
