# syntax=docker/dockerfile:1.7
#
# One Dockerfile, two images:
#   nie              BIN=nie              PACKAGE=nie-cli          FEATURES=cloud   runs `nie serve`
#   nie-model-serve  BIN=nie-model-serve  PACKAGE=nie-model-serve  FEATURES=host    runs `nie-model-serve`
#
# Local build from source (needs the vendor/ submodules in the context):
#   docker build -f docker/Dockerfile -t nie .
#   docker build -f docker/Dockerfile -t nie-model-serve \
#     --build-arg BIN=nie-model-serve --build-arg PACKAGE=nie-model-serve --build-arg FEATURES=host .
# CI build from a binary compiled on the same glibc baseline (release workflow):
#   docker build -f docker/Dockerfile --build-arg BIN_SOURCE=prebuilt ...   (binary in dist/<BIN>)
#
# No game data is baked in: mount the installation read-only and point NIE_GAME_SOURCE at it.

ARG DEBIAN_IMAGE=debian:bookworm-slim@sha256:734a72b3d6142a71993b4bc580ca01d8ca687f77a2c422d96a961d3376b179bc
ARG RUST_IMAGE=rust:1.100.0-slim-bookworm@sha256:7c7d4eb4d05b7781187a327809466cfddf29753f6defdc3d19a0e291951b7a37
ARG RCLONE_IMAGE=rclone/rclone:1.76.0@sha256:7cbeec24a44f206067597c0368be4de90ae19a64d5113c7d1938bd737a81e466
ARG BIN_SOURCE=compiled

FROM ${RCLONE_IMAGE} AS rclone

FROM ${RUST_IMAGE} AS builder
ENV RUSTUP_TOOLCHAIN=1.100.0
ARG PACKAGE=nie-cli
ARG BIN=nie
ARG FEATURES=cloud
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    CARGO_PROFILE_RELEASE_DEBUG=false CARGO_PROFILE_RELEASE_STRIP=true \
    cargo build --locked --release -p "${PACKAGE}" --no-default-features --features "${FEATURES}" --bin "${BIN}" \
    && install -D "target/release/${BIN}" "/out/${BIN}"

FROM scratch AS bin-compiled
COPY --from=builder /out/ /out/

FROM scratch AS bin-prebuilt
COPY dist/ /out/

FROM bin-${BIN_SOURCE} AS binsrc

FROM ${DEBIAN_IMAGE} AS runtime
ARG BIN=nie
# ssh: `ssh://` game sources; rclone: `r2://`, `s3://`, `gdrive://`, `rclone://` sources;
# curl: the container healthcheck.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl openssh-client \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --user-group --create-home --home-dir /home/nie --shell /usr/sbin/nologin nie \
    && install -d -o nie -g nie -m 0750 /var/lib/nie /var/lib/nie/model-cache \
    && install -d -m 0755 /game
COPY --from=rclone /usr/local/bin/rclone /usr/local/bin/rclone
# --chmod: a prebuilt binary arrives through a CI artifact, which drops the executable bit.
COPY --chmod=0755 --from=binsrc /out/${BIN} /usr/local/bin/${BIN}
COPY --chmod=0755 docker/entrypoint.sh /usr/local/bin/nie-entrypoint

ENV NIE_IMAGE_BIN=${BIN} \
    NIE_HOST=0.0.0.0 \
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
ENTRYPOINT ["/usr/local/bin/nie-entrypoint"]
