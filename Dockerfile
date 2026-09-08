# santui-server image.
# Multi-stage: Rust builder, Debian slim runtime. The radio station catalog
# (native/radio_stream_stations.db) is baked in — catalog updates ship with
# new images, which is fine because the DB is versioned with releases.
FROM rust:1-bookworm AS builder
WORKDIR /build
# Manifests first for layer caching; full source needed (workspace build).
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY native ./native
RUN cargo build --release -p santui-server

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates sqlite3 \
    && rm -rf /var/lib/apt/lists/*
# sqlite3 is for the backup CronJob, which reuses this image.
RUN useradd -r -u 10001 -m -d /data santui
COPY --from=builder /build/target/release/santui-server /usr/local/bin/santui-server
COPY --from=builder /build/native/radio_stream_stations.db /app/stations/radio_stream_stations.db
USER santui
ENV SANTUI_SERVER_HOST=0.0.0.0 \
    SANTUI_SERVER_PORT=9876 \
    SANTUI_SERVER_DATA_DIR=/data \
    SANTUI_STATIONS_DB=/app/stations/radio_stream_stations.db
VOLUME /data
EXPOSE 9876
ENTRYPOINT ["/usr/local/bin/santui-server"]
