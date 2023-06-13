FROM lukemathwalker/cargo-chef:latest-rust-1.70.0 as chef
WORKDIR /mallchat
COPY ./doc/config.cn.toml ./.cargo/config.toml

FROM chef as planner
COPY . .
# Compute a lock-like file for our project
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder

MAINTAINER "Geng Teng"

COPY --from=planner /mallchat/recipe.json recipe.json
# Build our project dependencies, not our application!
RUN cargo chef cook --release --no-default-features --recipe-path recipe.json
COPY . .
# Build our project
RUN cargo build --release --no-default-features

FROM debian:buster-slim AS target
WORKDIR /mallchat
COPY --from=builder /mallchat/target/release/mallchat .
COPY --from=builder /mallchat/server.example.toml server.toml

ENTRYPOINT ["./mallchat"]