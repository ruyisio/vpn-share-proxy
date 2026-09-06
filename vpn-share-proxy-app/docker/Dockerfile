FROM android-dev-env:latest

ENV ANDROID_HOME=/opt/android-sdk
ENV PATH="${PATH}:/opt/android-sdk/cmdline-tools/latest/bin:/opt/android-sdk/platform-tools:/opt/gradle-8.5/bin"
ENV GRADLE_USER_HOME=/gradle-cache

# Tools + Gradle once (cached unless this layer changes)
RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    unzip \
    && rm -rf /var/lib/apt/lists/* \
    && curl -fsSL https://services.gradle.org/distributions/gradle-8.5-bin.zip -o /tmp/gradle.zip \
    && unzip -q /tmp/gradle.zip -d /opt \
    && rm /tmp/gradle.zip

WORKDIR /workspace

# Build happens at `docker run` time with a persistent gradle cache volume.
# Do NOT run assemble here — COPY invalidates and would wipe all caches every change.
CMD ["sh", "-c", "gradle assembleDebug --stacktrace && mkdir -p /output && cp app/build/outputs/apk/debug/*.apk /output/vpn-share-proxy-app.apk && ls -lh /output"]
