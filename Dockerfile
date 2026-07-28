# ── Stage 1: Builder ────────────────────────────────────────────────────
FROM debian:trixie-slim AS builder

ARG TARGETARCH
ARG VERSION=v0.6.3

LABEL version=${VERSION}
LABEL org.opencontainers.image.version=${VERSION}

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy CI-downloaded binaries + ORT shared libs (arch-specific subfolder).
COPY binaries/ ./

# Select arch-specific binaries and ORT libraries
RUN if [ "$TARGETARCH" = "arm64" ]; then \
      mv uteke-arm64 uteke && mv uteke-serve-arm64 uteke-serve && mv uteke-mcp-arm64 uteke-mcp && \
      rm -f uteke-amd64 uteke-serve-amd64 uteke-mcp-amd64 && \
      if [ -d ort-arm64 ]; then cp ort-arm64/* . ; fi && \
      rm -rf ort-amd64 ort-arm64; \
    else \
      mv uteke-amd64 uteke && mv uteke-serve-amd64 uteke-serve && mv uteke-mcp-amd64 uteke-mcp && \
      rm -f uteke-arm64 uteke-serve-arm64 uteke-mcp-arm64 && \
      if [ -d ort-amd64 ]; then cp ort-amd64/* . ; fi && \
      rm -rf ort-amd64 ort-arm64; \
    fi && \
    chmod +x uteke uteke-serve uteke-mcp

# ── Stage 2: Runtime ────────────────────────────────────────────────────
FROM debian:trixie-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3t64 libstdc++6 curl && \
    rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN groupadd --system --gid 1000 uteke && \
    useradd --system --uid 1000 --gid uteke --home /data uteke

# Copy binaries
COPY --from=builder /build/uteke /usr/local/bin/uteke
COPY --from=builder /build/uteke-serve /usr/local/bin/uteke-serve
COPY --from=builder /build/uteke-mcp /usr/local/bin/uteke-mcp

# Copy ORT shared libraries (needed since we use load-dynamic, not download-binaries).
# These are placed in /usr/local/lib where ldconfig will find them.
COPY --from=builder /build/libonnxruntime*.so* /usr/local/lib/
COPY --from=builder /build/libonnxruntime_providers_shared.so* /usr/local/lib/
RUN ldconfig

# Copy entrypoint script (handles lazy model download on first run)
COPY docker-entrypoint.sh /usr/local/bin/entrypoint.sh
RUN chmod +x /usr/local/bin/entrypoint.sh

# Data directory (mount volume here for persistence)
ENV UTEKE_HOME=/data

# Create data directory with correct ownership
RUN mkdir -p /data && chown uteke:uteke /data

USER uteke

EXPOSE 8767

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
