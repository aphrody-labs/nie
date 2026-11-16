#!/bin/sh
# SPDX-License-Identifier: Apache-2.0
# Apply step of infra/systemd/rosegriffon-stack.service and the RG_* env of the prod host.
# Run on the host from the stack checkout (the deploy does it): installs the unit, records the
# image tag, drops legacy env keys, enables the unit. Idempotent. Usage: install-stack-unit.sh <RG_TAG>
set -eu
tag=${1:?usage: install-stack-unit.sh <RG_TAG>}
root=$(cd "$(dirname "$0")/../.." && pwd)
s=; [ "$(id -u)" = 0 ] || s=sudo
$s install -d -m 0750 /etc/rosegriffon
printf 'RG_TAG=%s\n' "$tag" | $s tee /etc/rosegriffon/stack.env >/dev/null
# Legacy keys the stack no longer reads (compose sets RG_REPO_ROOT).
for f in /etc/rosegriffon/rosegriffon.env /etc/rosegriffon/rosegriffon.build.env; do
  [ -f "$f" ] && $s sed -i '/^APHRODY_[A-Z_]*=/d' "$f"
done
$s install -m 0644 "$root/infra/systemd/rosegriffon-stack.service" /etc/systemd/system/rosegriffon-stack.service
$s systemctl daemon-reload
$s systemctl enable --now rosegriffon-stack.service >/dev/null
