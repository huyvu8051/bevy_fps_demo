# Builds crates/server only — the dedicated headless game server, not the
# client (which needs a GPU/window and isn't meant to run in a container).
# cargo-chef splits dependency compilation (cached across builds as long as
# Cargo.lock doesn't change) from app-code compilation (rebuilds every
# push) — this workspace pulls in Bevy/avian3d/big-brain/vleue_navigator,
# a genuinely large dependency tree, so skipping this caching would make
# every push a from-scratch multi-minute rebuild.
FROM rust:1.95.0-slim-bookworm AS chef
WORKDIR /app
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    wget \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN wget https://github.com/cargo-bins/cargo-binstall/releases/latest/download/cargo-binstall-x86_64-unknown-linux-musl.tgz \
    && tar -xvf cargo-binstall-x86_64-unknown-linux-musl.tgz \
    && cp cargo-binstall /usr/local/cargo/bin
RUN cargo binstall -y cargo-chef

FROM chef AS planner
COPY . .
# `prepare` doesn't take a -p/--package filter (unlike `cook`/`build` below)
# — it always plans the whole workspace, which is fine since the client
# crate's deps still get computed/cached even though nothing here builds it.
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# vendor/big-brain and vendor/vleue_navigator are local path dependencies
# (excluded from the workspace, not crates.io/git deps) — cargo-chef can
# stub out registry/git deps with empty source files during `cook`, but a
# path dependency's own real Cargo.toml has to actually be on disk for
# cargo to resolve it at all, or `cook` fails with "failed to get
# `big-brain` as a dependency" before it even gets to compiling anything.
COPY vendor vendor
RUN cargo chef cook --release --recipe-path recipe.json -p server
COPY . .
RUN cargo build --release -p server

FROM debian:bookworm-slim AS runner
RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/server /app/server

# renet2/netcode transport — see crates/server/src/main.rs's PORT constant.
EXPOSE 5031/udp

CMD ["/app/server"]
