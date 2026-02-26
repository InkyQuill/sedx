# Multi-stage Dockerfile for SedX

# Build stage
FROM docker.io/library/rust:1.75-alpine AS builder

WORKDIR /build

# Install build dependencies
RUN apk add --no-cache musl-dev

# Copy source
COPY . .

# Build release binary
RUN cargo build --release

# Runtime stage
FROM docker.io/library/alpine:3.19

# Install runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    bash

# Copy binary from builder
COPY --from=builder /build/target/release/sedx /usr/local/bin/sedx

# Install man page
RUN mkdir -p /usr/local/share/man/man1
COPY --from=builder /build/man/sedx.1 /usr/local/share/man/man1/

# Create non-root user
RUN addgroup -S sedx && \
    adduser -S sedx -G sedx

# Set working directory
WORKDIR /workdir

# Switch to non-root user
USER sedx

# Verify installation
RUN sedx --version

ENTRYPOINT ["/usr/local/bin/sedx"]
CMD ["--help"]
