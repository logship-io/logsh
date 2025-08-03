# Multi-stage build for logsh CLI tool
FROM rust:1.82-alpine as builder

# Install necessary build tools for musl target
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig

# Set working directory
WORKDIR /usr/src/app

# Copy the entire project
COPY . .

# Build the application without self-update feature using musl for static linking
WORKDIR /usr/src/app/logsh

# Set environment variables for static linking
ENV RUSTFLAGS="-C target-feature=+crt-static" \
    OPENSSL_STATIC=1

RUN cargo build --release --no-default-features --target x86_64-unknown-linux-musl

# Runtime stage - scratch image
FROM scratch

# Copy SSL certificates for HTTPS requests
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /usr/src/app/logsh/target/x86_64-unknown-linux-musl/release/logsh /logsh

# LOGSH_CONFIG_PATH - Override the default config file path
ENV LOGSH_CONFIG_PATH=/config/logsh-config.json

# Set the entrypoint
ENTRYPOINT ["/logsh"]
CMD ["--help"]