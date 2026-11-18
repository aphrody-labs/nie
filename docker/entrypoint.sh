#!/bin/sh
# Runs the binary this image ships: `nie serve` or `nie-model-serve`. Extra arguments are
# appended, so `docker run <image> --strict` becomes `nie serve --strict`.
set -eu
case "${NIE_IMAGE_BIN:?NIE_IMAGE_BIN is not set}" in
  nie) exec /usr/local/bin/nie serve "$@" ;;
  *) exec "/usr/local/bin/${NIE_IMAGE_BIN}" "$@" ;;
esac
