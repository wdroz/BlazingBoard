# Build stage
FROM rust:bookworm
RUN apt-get update && apt-get install -y \
    curl \
    gcc \
    chromium \
    ca-certificates \
    tzdata \
    openssl

WORKDIR /app
COPY . .
RUN cargo build

RUN cp /app/target/debug/content_updater /content_updater

CMD ["/content_updater"]