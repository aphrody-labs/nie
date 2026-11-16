# syntax=docker/dockerfile:1.12
# SPDX-License-Identifier: Apache-2.0
#
# Rose Griffon stack: one multi-stage file, one target per service of compose.yaml.
# Build context: repository root (see .dockerignore).
#
#   website      apps/rg-website, Next standalone on Bun          :3100
#   core         apps/rg-bot/src/core.ts: Discord gateways (rg, azalee, achillea),
#                ranked Colyseus, unified API /v1, bot admin       :8810
#   cron         packages/rg-cron/src/scheduler.ts (Bun.cron)     :8811, opt-in
#   nie          crates/nie-cli (--features cloud), musl  :8790
#   model-serve  crates/nie-model-serve, static musl      :8792
#
# Policy: Alpine runtimes, pinned versions, non-root uid 1000 (matches the host `ubuntu`
# owner of the /srv/rosegriffon bind mounts), no secret in any layer. Build-time NEXT_PUBLIC_*
# values come from the BuildKit secret `rg_env` and are only read inside one RUN.

ARG BUN_VERSION=canary
ARG ALPINE_VERSION=3.24
ARG RUST_IMAGE=rust:1-alpine3.24
ARG GO_IMAGE=golang:1.27-alpine
# rust-toolchain.toml says `nightly`; pin a dated nightly here for reproducible images.
ARG RUST_TOOLCHAIN=nightly

# ---------------------------------------------------------------------------
# Bun: workspace manifests only, so dependency layers survive source edits.
# ---------------------------------------------------------------------------
FROM oven/bun:${BUN_VERSION}-alpine AS manifests
WORKDIR /src
COPY . .
RUN mkdir /out \
    && cp package.json bun.lock bunfig.toml /out/ \
    && find apps packages -name package.json -not -path "*/node_modules/*" \
       -exec sh -c 'mkdir -p "/out/$(dirname "$1")" && cp "$1" "/out/$1"' _ {} \;

FROM oven/bun:${BUN_VERSION}-alpine AS bun-base
ENV NEXT_TELEMETRY_DISABLED=1 \
    BUN_INSTALL_CACHE_DIR=/root/.bun/install/cache
WORKDIR /app

# ---------------------------------------------------------------------------
# website
# ---------------------------------------------------------------------------
FROM bun-base AS website-deps
COPY --from=manifests /out/ ./
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile --ignore-scripts --filter @rosegriffon/website

FROM website-deps AS website-build
COPY tsconfig.json tsconfig.base.json turbo.json ./
COPY scripts ./scripts
COPY packages ./packages
COPY apps/rg-website ./apps/rg-website
ENV NODE_ENV=production
# `rg_env` is the production env file (NEXT_PUBLIC_* are inlined into the client bundle by
# design); it is sourced in this RUN only and never written to a layer.
RUN --mount=type=secret,id=rg_env,required=false \
    --mount=type=cache,target=/app/apps/rg-website/.next/cache \
    set -a; if [ -f /run/secrets/rg_env ]; then . /run/secrets/rg_env; fi; set +a; \
    bun run --cwd apps/rg-website build

FROM oven/bun:${BUN_VERSION}-alpine AS website
ENV NODE_ENV=production \
    NEXT_TELEMETRY_DISABLED=1 \
    HOSTNAME=0.0.0.0 \
    PORT=3100
WORKDIR /app
COPY --from=website-build --chown=bun:bun /app/apps/rg-website/.next/standalone ./
COPY --from=website-build --chown=bun:bun /app/apps/rg-website/.next/static ./apps/rg-website/.next/static
COPY --from=website-build --chown=bun:bun /app/apps/rg-website/public ./apps/rg-website/public
USER 1000:1000
WORKDIR /app/apps/rg-website
EXPOSE 3100
# Liveness: the process answers. /api/health returns 503 when Postgres is down, which is a
# dependency outage, not a reason to restart the container, so 503 counts as alive.
HEALTHCHECK --interval=15s --timeout=5s --start-period=40s --retries=5 \
    CMD ["bun", "-e", "fetch('http://127.0.0.1:'+(process.env.PORT||3100)+'/api/health').then(r=>process.exit(r.status<500||r.status===503?0:1),()=>process.exit(1))"]
CMD ["bun", "server.js"]

# ---------------------------------------------------------------------------
# core (API + bots) and cron share one production dependency tree
# ---------------------------------------------------------------------------
FROM bun-base AS server-deps
COPY --from=manifests /out/ ./
RUN --mount=type=cache,target=/root/.bun/install/cache \
    bun install --frozen-lockfile --ignore-scripts --production \
      --filter @rosegriffon/bot --filter @rosegriffon/cron

FROM oven/bun:${BUN_VERSION}-alpine AS server-base
ENV NODE_ENV=production
WORKDIR /app
COPY --from=server-deps --chown=bun:bun /app/ ./
COPY --chown=bun:bun tsconfig.json tsconfig.base.json ./
COPY --chown=bun:bun packages ./packages
# Cron tasks drive a headless Chrome through Bun.WebView (packages/rg-cron/src/lib/webview.ts);
# they run in core (default) or in the cron service.
ARG CHROMIUM_VERSION=156.0.8078.79-r0
ARG FONT_NOTO_VERSION=2026.11.01-r0
RUN apk add --no-cache chromium=${CHROMIUM_VERSION} font-noto=${FONT_NOTO_VERSION} \
    && mkdir -p /var/lib/rg-core /data/cdn && chown bun:bun /var/lib/rg-core /data/cdn \
    && ln -sfn ../../packages/rg-config node_modules/@rosegriffon/config
# Bun takes the tsconfig of the working directory, not of the entry file: without this, the
# Colyseus @type field decorators get TC39 semantics and crash. BUN_OPTIONS reaches the bot
# child processes too. The link restores @rosegriffon/config (a devDependency) for `extends`.
ENV BUN_OPTIONS=--tsconfig-override=/app/apps/rg-bot/tsconfig.json
ENV BUN_CHROME_PATH=/usr/bin/chromium

FROM server-base AS core
COPY --chown=bun:bun apps/rg-bot ./apps/rg-bot
# nie-model-serve runs as its own service; rg-core relays the model hosts to
# NIE_MODEL_SERVE_URL instead of supervising a child. Cron: RG_CORE_CRON (compose.yaml).
ENV RG_CORE_HOST=0.0.0.0 \
    RG_CORE_PORT=8810 \
    RG_CORE_MODELES=0
USER 1000:1000
WORKDIR /var/lib/rg-core
EXPOSE 8810
HEALTHCHECK --interval=15s --timeout=5s --start-period=60s --retries=5 \
    CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8810/health"]
CMD ["bun", "/app/apps/rg-bot/src/core.ts"]

FROM server-base AS cron
ENV RG_CRON_PORT=8811
USER 1000:1000
WORKDIR /var/lib/rg-core
EXPOSE 8811
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8811/health"]
CMD ["bun", "/app/packages/rg-cron/src/scheduler.ts"]

# ---------------------------------------------------------------------------
# nie-rclone: pure Go static binary (R2 / S3 / Google Drive VFS transport)
# ---------------------------------------------------------------------------
FROM ${GO_IMAGE} AS nie-rclone-build
WORKDIR /src
COPY native/go/rclone-core/go.mod native/go/rclone-core/go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
    go mod download
COPY native/go/rclone-core ./
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=0 go build -trimpath -ldflags="-s -w" -o /out/nie-rclone ./cmd/nie-rclone \
    && ! ldd /out/nie-rclone 2>/dev/null | grep -q "=>"

# ---------------------------------------------------------------------------
# Shared Rust musl builder base for nie and model-serve
# ---------------------------------------------------------------------------
FROM ${RUST_IMAGE} AS rust-base
ARG RUST_TOOLCHAIN
ENV RUSTUP_TOOLCHAIN=${RUST_TOOLCHAIN} \
    CARGO_TERM_COLOR=never
SHELL ["/bin/ash", "-eo", "pipefail", "-c"]
RUN apk add --no-cache musl-dev=1.2.6-r3 pkgconf=2.5.1-r0 perl=5.42.3-r0 make=4.4.1-r4 git=2.54.1-r0 \
    && rustup toolchain install "${RUST_TOOLCHAIN}" --profile minimal
WORKDIR /src
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates ./crates

# ---------------------------------------------------------------------------
# model-serve: static musl binary on bare Alpine
# ---------------------------------------------------------------------------
FROM rust-base AS model-serve-build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p nie-model-serve --features host \
    && install -D -m 0755 target/release/nie-model-serve /out/nie-model-serve \
    && ! ldd /out/nie-model-serve 2>/dev/null | grep -q "=>"

FROM alpine:${ALPINE_VERSION} AS model-serve
RUN addgroup -g 1000 -S nie && adduser -u 1000 -S -G nie -h /srv/nie nie \
    && mkdir -p /data/game /var/cache/nie-model-serve /srv/nie \
    && chown nie:nie /var/cache/nie-model-serve /srv/nie
COPY --from=model-serve-build /out/nie-model-serve /usr/local/bin/nie-model-serve
COPY --from=nie-rclone-build /out/nie-rclone /usr/local/bin/nie-rclone
ENV NIE_GAME_DIR=/data/game \
    NIE_MODEL_SERVE_HOST=0.0.0.0 \
    NIE_MODEL_SERVE_PORT=8792 \
    NIE_MODEL_CACHE_DIR=/var/cache/nie-model-serve \
    NIE_MODEL_SERVE_THREADS=8 \
    NIE_WORK_DIR=/srv/nie \
    NIE_RCLONE_BIN=/usr/local/bin/nie-rclone \
    RCLONE_CACHE_DIR=/var/cache/nie-model-serve
USER 1000:1000
WORKDIR /srv/nie
EXPOSE 8792
HEALTHCHECK --interval=15s --timeout=5s --start-period=60s --retries=5 \
    CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8792/health"]
# Env NIE_GAME_SOURCE / NIE_GAME_DIR resolve inside the binary so cloud URIs override /data/game cleanly.
CMD ["sh", "-c", "exec nie-model-serve --port \"${NIE_MODEL_SERVE_PORT:-8792}\" --cache-dir \"${NIE_MODEL_CACHE_DIR:-/var/cache/nie-model-serve}\" --threads \"${NIE_MODEL_SERVE_THREADS:-8}\" $NIE_MODEL_SERVE_ARGS"]

# ---------------------------------------------------------------------------
# nie: pure-Rust cloud HTTP server + VFS/format CLI on bare Alpine
# ---------------------------------------------------------------------------
FROM rust-base AS nie-build
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release -p nie-cli --no-default-features --features cloud --bin nie \
    && install -D -m 0755 target/release/nie /out/nie \
    && ! ldd /out/nie 2>/dev/null | grep -q "=>"

FROM alpine:${ALPINE_VERSION} AS nie
RUN addgroup -g 1000 -S nie && adduser -u 1000 -S -G nie -h /srv/nie nie \
    && mkdir -p /data/game /var/cache/nie /srv/nie \
    && chown nie:nie /var/cache/nie /srv/nie
COPY --from=nie-build /out/nie /usr/local/bin/nie
COPY --from=nie-rclone-build /out/nie-rclone /usr/local/bin/nie-rclone
ENV NIE_HOST=0.0.0.0 \
    NIE_PORT=8790 \
    NIE_WORK_DIR=/srv/nie \
    NIE_RCLONE_BIN=/usr/local/bin/nie-rclone \
    RCLONE_CACHE_DIR=/var/cache/nie
USER 1000:1000
WORKDIR /srv/nie
EXPOSE 8790
HEALTHCHECK --interval=15s --timeout=5s --start-period=20s --retries=5 \
    CMD ["wget", "-q", "-O", "/dev/null", "http://127.0.0.1:8790/health"]
CMD ["sh", "-c", "exec nie serve --host \"${NIE_HOST:-0.0.0.0}\" --port \"${NIE_PORT:-8790}\" $NIE_ARGS"]
