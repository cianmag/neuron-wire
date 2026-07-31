# ─── Multi-stage Docker build for neuron-wire node binary ─────────
# Builds a tiny (<10MB), stripped, production binary on Alpine Linux.
#
# Usage:
#   docker build -t neuron-wire-node .
#   docker run -it --rm \
#     -v ./node-config.toml:/etc/nwp/node-config.toml:ro \
#     -v ./identity.key:/etc/nwp/identity.key \
#     -p 9000:9000/udp \
#     neuron-wire-node \
#       --config /etc/nwp/node-config.toml \
#       --identity /etc/nwp/identity.key
#
# To cross-compile from a non-Linux host, run instead:
#   cargo build --release --bin node
#   docker build -f Dockerfile.scratch . -t neuron-wire-node

# ── Builder stage ─────────────────────────────────────────────────
FROM rust:1.87-alpine AS builder
LABEL stage=builder

# Build dependencies for static linking
RUN apk add --no-cache musl-dev

# Create empty project for dependency caching
WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY demo/Cargo.toml demo/
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --bin node 2>/dev/null || true
RUN rm -rf src

# Real source
COPY src ./src
COPY demo ./demo

# Touch main.rs to force rebuild of the binary (cached deps stay fresh)
RUN touch src/bin/node.rs && \
    cargo build --release --bin node 2>&1

# ── Runtime stage ─────────────────────────────────────────────────
FROM alpine:3.20 AS runtime

# Runtime dependencies (musl, ca-certificates for STUN, tzdata for logs, wget for healthcheck)
RUN apk add --no-cache ca-certificates tzdata wget

# Create non-root user
RUN addgroup -S nwp && adduser -S nwp -G nwp

# Binary
COPY --from=builder /build/target/release/node /usr/local/bin/nwp-node

# Config and identity are mounted at runtime (bind mounts)
RUN mkdir -p /etc/nwp && chown nwp:nwp /etc/nwp

USER nwp
WORKDIR /etc/nwp

EXPOSE 9000/udp
EXPOSE 9100/tcp

# Health check: poll the HTTP health endpoint every 10s
# Docker marks container as unhealthy after 3 consecutive failures (30s)
HEALTHCHECK --interval=10s --timeout=3s --start-period=15s --retries=3 \
    CMD wget -qO- http://localhost:9100/health || exit 1

ENTRYPOINT ["nwp-node"]
CMD ["--config", "./node-config.toml", "--identity", "./identity.key"]
