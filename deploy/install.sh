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

systemctl disable --now rg-nie-xvfb.service 2>/dev/null || true
install -m 0755 "$here/bin/nie-xvfb" /usr/local/bin/nie-xvfb
install -m 0755 "$here/bin/nie-run" /usr/local/bin/nie-run
install -m 0644 "$here"/systemd/*.service /etc/systemd/system/
systemctl daemon-reload

docker compose -f /srv/nie/deploy/compose.yaml --env-file /etc/nie/stack.env pull
systemctl enable --now nie-stack.service nie-xvfb.service
if [ -f /etc/nie/cloudflared.token ]; then systemctl enable --now nie-cloudflared.service; fi
systemctl restart nie-stack.service
