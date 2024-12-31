FROM rust:1 AS builder

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall dioxus-cli --root /.cargo -y --force
WORKDIR /app
COPY . .
WORKDIR /app/blazing_board
RUN /.cargo/bin/dx bundle --platform web

FROM rust:1 AS runtime
COPY --from=builder /app/blazing_board/target/dx/blazing_board/release/web/ /usr/local/app

# set our port and make sure to listen for all connections
ENV PORT=8080
ENV IP=0.0.0.0

# expose the port 8080
EXPOSE 8080

WORKDIR /usr/local/app
ENTRYPOINT [ "/usr/local/app/server" ]

