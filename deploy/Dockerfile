# Multi-stage build for the organized-koalad server binary.
#
# Stage 1 compiles the workspace with sqlx in OFFLINE mode (SQLX_OFFLINE=true), so the
# committed .sqlx/ query cache stands in for a live database at compile time — the build
# needs no Postgres. The embedded migrations/ tree (sqlx::migrate!) ships inside the binary,
# so the runtime image carries its full schema and needs no sqlx CLI or checkout (ADR-0004).
#
# Stage 2 is a slim Debian runtime carrying only the binary and the TLS/libgcc runtime libs.

# --- build stage -------------------------------------------------------------------------
FROM rust:1.96.0-bookworm AS build

# sqlx compiles against the committed query cache, never a live DB.
ENV SQLX_OFFLINE=true

WORKDIR /build

# Copy the whole workspace. .dockerignore keeps target/ and other host cruft out of context.
COPY . .

# Build only the server binary in release mode.
RUN cargo build --release --bin organized-koalad

# --- runtime stage -----------------------------------------------------------------------
FROM debian:bookworm-slim AS runtime

# ca-certificates for outbound TLS (e.g. OTLP); curl for the compose liveness healthcheck
# (GET /healthz). libgcc/libssl are pulled in by the base.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

# Run as an unprivileged user.
RUN useradd --system --create-home --user-group koala
USER koala

COPY --from=build /build/target/release/organized-koalad /usr/local/bin/organized-koalad

# Default to the long-running server; compose overrides the command for the migrate one-shot.
ENTRYPOINT ["organized-koalad"]
CMD ["run"]
