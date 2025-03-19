FROM rust:1.85.0-slim-bookworm AS builder

RUN apt-get update && apt-get install -y \
    build-essential \
    curl \
    openssl \
    libssl-dev \
    pkg-config \
    libopus-dev

RUN curl -L https://yt-dl.org/downloads/latest/youtube-dl -o /usr/local/bin/youtube-dl\
 && chmod a+rx /usr/local/bin/youtube-dl

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=bind,source=.sqlx,target=.sqlx \
cargo install --path .  --locked


FROM debian:bookworm-slim

RUN <<EOF
set -e
# allow manpage installation
sed -i '/path-exclude \/usr\/share\/man/d' /etc/dpkg/dpkg.cfg.d/docker
sed -i '/path-exclude \/usr\/share\/groff/d' /etc/dpkg/dpkg.cfg.d/docker

# add non-free
sed -i 's/Components: main/Components: main non-free/' /etc/apt/sources.list.d/debian.sources
apt update
apt install -y curl libopus-dev man manpages-dev manpages-posix manpages-posix-dev
apt install --reinstall coreutils
rm -rf /var/lib/apt/lists/*
EOF

COPY --from=builder /usr/local/bin/youtube-dl /usr/local/bin/youtube-dl
COPY --from=builder /usr/local/cargo/bin/wobot /wobot

CMD ["/wobot"]
