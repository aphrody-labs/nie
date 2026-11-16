# Rose Griffon Docker stack

`compose.yaml` (repository root) runs every Rose Griffon process on the `prod` role host (prd).
Images come from [`Dockerfile`](./Dockerfile), one target per service. Roles and hosts are in
`infra/hosts.toml` (fr1 = build, fr2 = edge/cdn, prd = prod), services in `infra/services.toml`.

## Topology

```
Cloudflare --(origin rule: 8443, cacheable 9443)--> prd host nginx (TLS, host-nginx/rosegriffon-stack.conf)
                                                      -> 127.0.0.1:18080 stack nginx (routing, proxy_cache, rate limits)
                                                           -> website:3100  core:8810  model-serve:8792  nie:8790
cdn.rosegriffon.fr, nie.aphrody.app --> fr2 nginx :443 (/static from disk, rest cached) -> prd :8443
```

The host nginx owns TLS; the stack nginx publishes only `127.0.0.1:18080` and the mesh address
(`RG_MESH_IP`). Every other service sits on the internal `backend` network.

## Services

| Service       | Source                                                                              | Port | Health        |
| ------------- | ----------------------------------------------------------------------------------- | ---- | ------------- |
| `nginx`       | `nginxinc/nginx-unprivileged`, `infra/docker/nginx/`                                | 8080 | `/nginx-health` |
| `website`     | `apps/rg-website` (Next standalone, Bun)                                            | 3100 | `/api/health` |
| `core`        | `apps/rg-bot/src/core.ts`: Discord gateways, ranked, unified API `/v1`, cron (aliases `api`, `bot`) | 8810 | `/health`     |
| `model-serve` | `crates/nie-model-serve`, static musl                                       | 8792 | `/health`     |
| `nie`         | `crates/nie-cli` (cloud feature), profile `nie` (opt-in), static musl        | 8790 | `/health`     |
| `cron`        | `packages/rg-cron/src/scheduler.ts`, profile `cron-split` (opt-in)                  | 8811 | `/health`     |
| `redis`       | cache only, no persistence                                                          | 6379 | `redis-cli ping` |
| `inagle-pg`   | Postgres for Azalee/Inagle, mesh bind `RG_MESH_IP:15433`                            | 5432 | `pg_isready`  |
| `steam-depot` | profile `tools`, `infra/docker/steam-depot/`                                        | none | on demand     |

Cron runs inside core (`RG_CORE_CRON=1`); `cron-split` needs a cross-process event bus first.

## Host layout (prd, `RG_SRV_ROOT=/srv/rosegriffon`)

| Path                  | Use                                                                 |
| --------------------- | ------------------------------------------------------------------- |
| `game/ievr`           | game install (read-only in containers, written by steam-depot)      |
| `steam`               | steam-depot home (login token), 0700                                |
| `cdn/static`          | derived public assets, source of the R2/S3 and fr2 sync             |
| `cache/{nginx,nie,model-serve}` | proxy and GLB caches                                      |
| `state`               | `/var/lib/rg-core` (media, shared, disputes, ranked, backups)       |
| `nie`                 | model-serve working dir                                             |
| `db/inagle`           | Postgres data (uid 70)                                              |
| `log/nginx`           | nginx logs (`rg-stats.log` is read by the stats cron task)          |
| `secrets`             | service account files, read-only in containers                      |
| `src`                 | deploy checkout of this repository (`stack_dir`)                    |
| `/etc/rosegriffon/`   | `rosegriffon.env`, `rosegriffon.build.env`, `inagle-pg.env` (0640, never committed), `stack.env` (`RG_TAG`) |

All bind mounts; names only in [`.env.example`](./.env.example). Compose sets in-container
paths (`RG_REPO_ROOT`, `RG_BOT_DIR`, `NIE_*`, `RG_MEDIA_ROOT`) and overrides the env file.

## Deploy

```sh
just deploy [service|all] [--ref REF]   # fr1 builds (docker compose build), images shipped to prd (docker save | load), up -d --wait
just verify [service|all]               # health inside the stack network, stack nginx, public names
just rollback <tag> [service]           # previous images (tag = short commit of a good deploy)
```

A whole-stack deploy installs `infra/systemd/rosegriffon-stack.service` on prd
(`infra/scripts/install-stack-unit.sh`): `Restart=always`, enabled at boot, `RG_TAG` read from
`/etc/rosegriffon/stack.env`. The build host reads `rosegriffon.build.env` (loopback DB URLs, host
network) for the website prerender.

## Edge and sync (fr2, fr1)

- fr2 nginx: `infra/docker/cdn-edge/` (cdn.rosegriffon.fr, nie.aphrody.app).
- `rg-cdn-sync.timer` (`infra/systemd/`, `infra/scripts/sync-cdn.sh`): prd -> fr1 -> fr2 + R2 bucket `rosegriffon-cdn`.
- `rg-steam-depot.timer`: hourly game update on prd, lock `/var/lock/rosegriffon-heavy.lock`.
- prd ports: 8443 and 9443 (host nginx, Cloudflare ranges only), 80/443 belong to another tenant.
