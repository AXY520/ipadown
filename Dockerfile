# syntax=docker/dockerfile:1.7
# ---------- 构建 ----------
FROM rust:1-bookworm AS builder
ARG TARGETARCH
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN --mount=type=cache,id=ipadown-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=ipadown-target-${TARGETARCH},target=/app/target,sharing=locked \
    cargo build --locked --release -p ipadown-web \
    && cp /app/target/release/ipadown-web /tmp/ipadown-web

# ---------- 运行 ----------
FROM debian:bookworm-slim
ARG TARGETARCH
ARG IPATOOL_VERSION=2.6.0
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
# ipatool：SAP 签名登录/下载依赖（2026-08 起苹果强制 X-Apple-ActionSignature）
RUN arch="${TARGETARCH:-amd64}" \
    && case "$arch" in \
         amd64) expected="a8bbd8a5a6515d0b858c6cfe6c2baf70943a4235b7fa65d5bd01a42a7d5bedbb" ;; \
         arm64) expected="7c7ce6c54379683d77b99d39f1d1a766fd8de3ae09ae8c4c64f5d04232e6ae80" ;; \
         *) echo "unsupported architecture: $arch" >&2; exit 1 ;; \
       esac \
    && curl -sfL "https://github.com/majd/ipatool/releases/download/v${IPATOOL_VERSION}/ipatool-${IPATOOL_VERSION}-linux-${arch}.tar.gz" -o /tmp/ipatool.tgz \
    && echo "$expected  /tmp/ipatool.tgz" | sha256sum -c - \
    && tar xzf /tmp/ipatool.tgz -C /tmp \
    && mv "/tmp/bin/ipatool-${IPATOOL_VERSION}-linux-${arch}" /usr/local/bin/ipatool \
    && chmod +x /usr/local/bin/ipatool \
    && rm /tmp/ipatool.tgz
COPY --from=builder /tmp/ipadown-web /usr/local/bin/ipadown-web

ENV IPADOWN_DATA_DIR=/data \
    PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s \
    CMD curl -sf http://localhost:${PORT}/healthz || exit 1

CMD ["ipadown-web"]
