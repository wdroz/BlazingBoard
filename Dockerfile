FROM rust:trixie AS builder
RUN apt-get update && apt-get install -y \
    curl \
    build-essential \
    ca-certificates \
    tzdata \
    openssl \
    git

# --locked pins deps from dioxus-cli's Cargo.lock (avoids git2 0.21 / auth-git2 breakage)
RUN cargo install dioxus-cli --locked --root /.cargo

WORKDIR /app
COPY . .
WORKDIR /app/blazing_board
RUN /.cargo/bin/dx bundle --web --release

FROM rust:trixie AS runtime
COPY --from=builder /app/blazing_board/target/dx/blazing_board/release/web/ /usr/local/app

# dx always names the fullstack server binary "server" (not the crate name)
RUN test -x /usr/local/app/server && ls -la /usr/local/app

# set our port and make sure to listen for all connections
ENV PORT=8080
ENV IP=0.0.0.0

# expose the port 8080
EXPOSE 8080

WORKDIR /usr/local/app
ENTRYPOINT [ "/usr/local/app/server" ]

