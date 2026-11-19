# syntax=docker/dockerfile:1.7
#
# The one Dockerfile of the monorepo. One builder compiles every shipped binary; each image is a
# named target on a shared runtime base (Docker Docs, "Build dependent images", multi-stage):
#   docker build --target nie             -t nie .              # nie serve (cloud profile)
#   docker build --target nie-model-serve -t nie-model-serve .  # nie-model-serve (host profile)
#   docker compose build                                        # both, from compose.yaml
# CI builds from binaries compiled on the same glibc baseline (release workflow):
#   docker build --target <bin> --build-arg BIN_SOURCE=prebuilt .   (binaries in dist/<bin>)
#
# No game data is baked in: mount the installation read-only and point NIE_GAME_SOURCE at it.

ARG DEBIAN_IMAGE=debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
ARG RUST_IMAGE=rust:1.100.0-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
ARG RCLONE_IMAGE=rclone/rclone:1.76.0@sha256:7cbeec24a44f206067597c0368be4de90ae19a64d5113c7d1938bd737a81e466
ARG BIN_SOURCE=compiled

FROM ${RCLONE_IMAGE} AS rclone

FROM ${RUST_IMAGE} AS builder
ENV RUSTUP_TOOLCHAIN=1.100.0 \
    CARGO_PROFILE_RELEASE_DEBUG=false \
    CARGO_PROFILE_RELEASE_STRIP=true
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --locked --release --no-default-features -p nie-model-serve --features nie-model-serve/host --bin nie-model-serve \
    && cargo build --locked --release --no-default-features -p nie-cli --features nie-cli/cloud --bin nie \
    && install -D -t /out target/release/nie target/release/nie-model-serve

FROM scratch AS bin-compiled
COPY --from=builder /out/ /out/

FROM scratch AS bin-prebuilt
COPY dist/ /out/

FROM bin-${BIN_SOURCE} AS binsrc

FROM ${DEBIAN_IMAGE} AS runtime
# ssh: `ssh://` game sources; rclone: `r2://`, `s3://`, `gdrive://`, `rclone://` sources;
# curl: the container healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl openssh-client \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --user-group --create-home --home-dir /home/nie --shell /usr/sbin/nologin nie \
    && install -d -o nie -g nie -m 0750 /var/lib/nie /var/lib/nie/model-cache \
    && install -d -m 0755 /game
COPY --from=rclone /usr/local/bin/rclone /usr/local/bin/rclone
ENV NIE_HOST=0.0.0.0 \
    NIE_MODEL_SERVE_HOST=0.0.0.0 \
    NIE_PORT=8790 \
    NIE_CACHE_DIR=/var/lib/nie/model-cache \
    RCLONE_CONFIG=/home/nie/.config/rclone/rclone.conf
USER 10001:10001
WORKDIR /var/lib/nie
VOLUME ["/var/lib/nie"]
EXPOSE 8790
HEALTHCHECK --interval=30s --timeout=5s --start-period=60s --retries=3 \
    CMD curl --fail --silent --show-error --output /dev/null "http://127.0.0.1:${NIE_PORT}/health"

FROM runtime AS nie-model-serve
# --chmod: a prebuilt binary arrives through a CI artifact, which drops the executable bit.
COPY --chmod=0755 --from=binsrc /out/nie-model-serve /usr/local/bin/nie-model-serve
ENTRYPOINT ["/usr/local/bin/nie-model-serve"]

# Last stage: the default target. Extra arguments are appended: `docker run <image> --strict`.
FROM runtime AS nie
COPY --chmod=0755 --from=binsrc /out/nie /usr/local/bin/nie
ENTRYPOINT ["/usr/local/bin/nie", "serve"]
