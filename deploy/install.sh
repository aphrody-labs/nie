#!/usr/bin/env bash
# Install or update the nie deployment on the production host (run as root on prd):
#   NIE_TAG=v1.4.8 deploy/install.sh
# Layout, the Steam library of the game plus nie state:
#   /srv/nie/steam/steamapps/common/INAZUMA ELEVEN Victory Road   game, data/ is the VFS (read-only for uid 10001)
#   /srv/nie/steam/steamapps/compatdata/2799860/pfx                Proton prefix used by nie-run
#   /srv/nie/steam/steamapps/appmanifest_2799860.acf               Steam app manifest
#   /srv/nie/cache/{nie,nie-model}   /srv/nie/deploy   /etc/nie
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tag=${NIE_TAG:?set NIE_TAG, for example v1.4.8}

install -d -m 0755 /srv/nie/deploy /etc/nie
install -d -m 0755 -o 10001 -g 10001 /srv/nie/cache /srv/nie/cache/nie /srv/nie/cache/nie-model
game="/srv/nie/steam/steamapps/common/INAZUMA ELEVEN Victory Road"
setfacl -m u:10001:rx /srv/nie /srv/nie/steam /srv/nie/steam/steamapps /srv/nie/steam/steamapps/common "$game"
setfacl -d -m u:10001:rx "$game"
install -m 0644 "$here/../compose.yaml" /srv/nie/deploy/compose.yaml
printf 'NIE_TAG=%s\n' "$tag" > /etc/nie/stack.env

install -m 0755 "$here/bin/nie-xvfb" /usr/local/bin/nie-xvfb
install -m 0755 "$here/bin/nie-run" /usr/local/bin/nie-run
install -m 0644 "$here"/systemd/*.service "$here"/systemd/*.timer /etc/systemd/system/
systemctl daemon-reload

docker compose -f /srv/nie/deploy/compose.yaml --env-file /etc/nie/stack.env pull
# nie owns the game-side units that Rose Griffon ran before 2026-11-19.
for u in rg-nie-xvfb.service rg-steam-depot.timer rg-steam-depot.service rg-steam-fill.service; do
  systemctl disable --now "$u" 2>/dev/null || true
done
if [ -n "${NIE_RELEASE_TARBALL:-}" ]; then
  d=$(mktemp -d); tar xzf "$NIE_RELEASE_TARBALL" -C "$d"; install -m 0755 "$d"/*/nie /usr/local/bin/nie; rm -rf "$d"
fi
systemctl enable --now nie-stack.service nie-xvfb.service
if [ -f /srv/nie/steam/steam-tokens.json ] && [ -f /etc/nie/steam.env ]; then systemctl enable --now nie-steam-update.timer; fi
if [ -f /etc/nie/cloudflared.token ]; then systemctl enable --now nie-cloudflared.service; fi
systemctl restart nie-stack.service
