# Multi-stage multi-arch build for logsh CLI tool
FROM --platform=$BUILDPLATFORM rust:1.93.1-alpine AS builder

ARG TARGETARCH

# Install necessary build tools for musl target
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

# Install cross-compilation toolchains when needed
RUN case "$TARGETARCH" in \
      arm64) apk add --no-cache gcc-aarch64-none-elf ;; \
      arm)   apk add --no-cache gcc-arm-none-eabi ;; \
    esac || true

# Set working directory
WORKDIR /usr/src/app

# Copy the entire project
COPY . .
WORKDIR /usr/src/app/logsh

# Map Docker platform to Rust target and build
ENV RUSTFLAGS="-C target-feature=+crt-static" \
    OPENSSL_STATIC=1

RUN case "$TARGETARCH" in \
      amd64) RUST_TARGET="x86_64-unknown-linux-musl" ;; \
      arm64) RUST_TARGET="aarch64-unknown-linux-musl" ;; \
      arm)   RUST_TARGET="armv7-unknown-linux-musleabihf" ;; \
      *)     echo "Unsupported arch: $TARGETARCH" && exit 1 ;; \
    esac && \
    rustup target add "$RUST_TARGET" && \
    cargo fetch --target "$RUST_TARGET" && \
    cargo build --release --no-default-features --target "$RUST_TARGET" && \
    cp "target/$RUST_TARGET/release/logsh" /logsh-binary

# Runtime stage - scratch image
FROM scratch

# Copy SSL certificates for HTTPS requests
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /logsh-binary /logsh

# LOGSH_CONFIG_PATH - Override the default config file path
ENV LOGSH_CONFIG_PATH=/config/logsh-config.json

# Set the entrypoint
ENTRYPOINT ["/logsh"]
CMD ["--help"]
