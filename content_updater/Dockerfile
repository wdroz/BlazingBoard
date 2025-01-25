# Build stage
FROM messense/rust-musl-cross:x86_64-musl AS builder
WORKDIR /app
COPY . .
RUN cargo build

# content_updater runtime
FROM alpine:3 AS content_updater
RUN apk add --no-cache chromium ca-certificates tzdata
COPY --from=builder /app/target/x86_64-unknown-linux-musl/debug/content_updater /content_updater
CMD ["/content_updater"]