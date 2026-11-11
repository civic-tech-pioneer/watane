FROM rust:1.95-trixie
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
    libayatana-appindicator3-dev librsvg2-dev patchelf \
    xvfb xauth dbus-x11 openbox x11-utils nodejs ca-certificates \
    && rm -rf /var/lib/apt/lists/*
# Bound memory and disk use when compiling GTK and Wasmtime in Docker Desktop.
ENV CARGO_BUILD_JOBS=2 \
    CARGO_PROFILE_DEV_DEBUG=0 \
    CARGO_PROFILE_TEST_DEBUG=0 \
    CARGO_INCREMENTAL=0
