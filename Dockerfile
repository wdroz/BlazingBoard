FROM rust:bookworm AS builder
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    ca-certificates \
    tzdata \
    openssl \
    git

# Install dioxus-cli with specific features
RUN cargo install dioxus-cli --root /.cargo

WORKDIR /app
COPY . .
WORKDIR /app/blazing_board
RUN /.cargo/bin/dx bundle --platform web

FROM rust:bookworm AS runtime
COPY --from=builder /app/blazing_board/target/dx/blazing_board/release/web/ /usr/local/app

# set our port and make sure to listen for all connections
ENV PORT=8080
ENV IP=0.0.0.0

# expose the port 8080
EXPOSE 8080

WORKDIR /usr/local/app
ENTRYPOINT [ "/usr/local/app/server" ]

