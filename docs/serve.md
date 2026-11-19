# Running nie as a service

Two HTTP servers ship with every release. Neither carries game data: both read the installation
the operator owns, in place, through the VFS (`nie-formats`).

| image / binary | what it is for |
|---|---|
| `ghcr.io/aphrody-labs/nie` / `nie serve` | JSON API: VFS listing and lookup, format detection and decoding, game-data queries, the wiki (JSON and HTML), `/openapi.json` |
| `ghcr.io/aphrody-labs/nie-model-serve` / `nie-model-serve` | asset origin for a CDN: decoded textures (PNG), audio (WAV), video (MP4), cfg as JSON, models (GLB), raw bytes behind a token |

Release archives (`nie-v<version>-x86_64-unknown-linux-gnu.tar.gz`,
`nie-v<version>-x86_64-pc-windows-msvc.zip`) hold the full `host` build of `nie` and
`nie-model-serve`, plus `SHA256SUMS` on the release page. The images run the pure-Rust `cloud`
profile of `nie` (no Lua C, no SQLite) and the `host` profile of `nie-model-serve`.

## Configuration

Environment variables (a flag of the same name wins). The image sets `NIE_HOST=0.0.0.0` and
`NIE_PORT=8790`.

| variable | default | meaning |
|---|---|---|
| `NIE_HOST` | `127.0.0.1` | bind address of `nie serve` (`NIE_MODEL_SERVE_HOST` for `nie-model-serve`) |
| `NIE_PORT` | `8790` | TCP port, read by both servers |
| `NIE_GAME_SOURCE` | discovery | where the installation lives, see below; `NIE_GAME_DIR` is also read |
| `NIE_RAW_VFS_TOKEN` | unset | bearer token of at least 32 bytes that opens the game's untouched bytes; unset or shorter keeps them closed |
| `NIE_HTTP_AUTH_TOKEN` | unset | bearer sent to an `https://` source |
| `NIE_CACHE_DIR` | `<game>/var/model-cache` | `nie-model-serve` cache of assembled GLB, mixed WAV and MP4; `/var/lib/nie/model-cache` in the image |
| `RCLONE_CONFIG` | rclone default | rclone configuration for `r2://`, `s3://`, `gdrive://`, `rclone://` sources |

`nie serve` flags: `--strict` exits at startup when the source cannot be mounted; `--stateless`
makes `/ready` healthy without a VFS (format and registry worker mode).

### Game source (`NIE_GAME_SOURCE`)

| value | transport |
|---|---|
| `/game`, `D:\Games\ievr` | local files, CPK ranges read in place |
| `ssh://[user@]host[:port]/path`, `sftp://...` | the `ssh` client (host keys and credentials from `~/.ssh`) |
| `r2://remote/bucket/path`, `s3://`, `gdrive://remote/path`, `rclone://remote/path` | the `rclone` CLI (`rclone cat --offset --count`), remotes from `rclone.conf` |
| `https://host/path` | HTTP range requests |

Credentials never go in the URI. Only the CPK tables and the requested ranges are transferred.

### Container

One `Dockerfile` and one `compose.yaml` at the repository root build and run every image: the
targets `nie` (`nie serve`, cloud profile) and `nie-model-serve` (host profile) share one builder.

```
docker build --target nie -t nie .
NIE_TAG=dev NIE_GAME_DIR=/path/to/game NIE_CACHE_ROOT=./var docker compose up --build
```

### Production host

`NIE_TAG=v2.0.0 deploy/install.sh` (root on prd) installs compose project `nie` under
`/srv/nie/deploy` with `nie-stack.service`, `nie-xvfb.service` and, once
`/etc/nie/cloudflared.token` exists, `nie-cloudflared.service` for `nie.aphrody.app`. `nie serve`
listens on `127.0.0.1:8796`, `nie-model-serve` on `127.0.0.1:8797`. `/srv/nie/steam` is the Steam
library of the game: `steamapps/common/INAZUMA ELEVEN Victory Road` (mounted read-only, uid 10001
ACL), `steamapps/compatdata/2799860/pfx` (the Proton prefix `nie-run` starts `nie.exe` in) and
`steamapps/appmanifest_2799860.acf`.

Remote sources: mount `rclone.conf` at `/home/nie/.config/rclone/rclone.conf` and `~/.ssh` at
`/home/nie/.ssh` (read-only, readable by uid 10001). The image runs as uid 10001, exposes 8790,
keeps writable state in the `/var/lib/nie` volume and has a `/health` healthcheck. Extra
arguments are appended to the command: `docker run ... nie --strict` runs `nie serve --strict`.

Without a reachable source `nie serve` still starts (`/health` answers, `/ready` returns 503
until `--stateless`), and every VFS route answers 503; `--strict` turns that into an exit.

## `nie serve` routes

All responses carry `Access-Control-Allow-Origin: *` and `X-Content-Type-Options: nosniff`.
`HEAD` works on every `GET` route, `OPTIONS` answers 204. Errors are
`{"error": string}` (VFS routes add `"reason"` or `"path"`). Bodies are limited to 32 MiB.

| route | answer |
|---|---|
| `GET /health`, `GET /` | 200 `{ok, service:"nie", profile:"cloud"\|"host", vfsReady, stateless, source, vfsError, entries, cpks}` |
| `GET /ready` | 200 `{ready:true, vfsReady, stateless}`; 503 `{ready:false, vfsReady:false, reason}` |
| `GET /v1/vfs/remotes` | 200 `{remotes}` (rclone remotes); 503 without rclone |
| `POST /v1/formats/detect` | body = bytes; 200 `{format, size}` |
| `POST /v1/formats/decode?name=&format=` | body = bytes; `format=png` returns `image/png` (G4TX), else the decoded structure as JSON; 422 when unreadable |
| `GET /v1/vfs/info` | 200 `{source, entries, cpks, transport:{ioCalls, payloadBytes}}`; 503 without VFS |
| `GET /v1/vfs/find?q=&ext=&limit=` | 200 `{count, items:[{path, size, cpk}]}`, `limit` 1 to 500 (default 50) |
| `GET /v1/vfs/stat?path=` | 200 `{path, size, cpk}`; 400 invalid path, 404 unknown |
| `GET /v1/vfs/read?path=&decode=` | `decode=json`: decoded structure (`application/json`, `max-age=300`); `decode=png`: texture as `image/png`; otherwise the raw bytes as `application/octet-stream`, which needs `Authorization: Bearer <NIE_RAW_VFS_TOKEN>` and is a 404 for everyone while no token is configured; 422 when not decodable |
| `GET /v1/game/query?domain=&q=&limit=` | 200 game-data rows of `nie_explore::domain_data::query_vfs` (default domain `characters`), 400 on an unknown domain |
| `GET /openapi.json` | the OpenAPI 3.1 description of every route in this table, generated from the same route list the router dispatches on (`crates/nie-cli/src/serve_routes.rs`; a test compares the two) |
| `GET /v1/wiki/languages` | 200 `{count, items:[{folder, tag}]}`: the folders of `data/common/text` that hold the character names, with their BCP 47 tags |
| `GET /v1/wiki/unsourced` | 200 `{count, items:[{family, reason}]}`: the families the VFS gives no source for |
| `GET /v1/wiki/{characters,skills,teams}?lang=&q=&limit=&offset=` | 200 `{total, offset, limit, items}`; `lang` takes folders or BCP 47 tags (`de en es fr it ja pt zh-Hans zh-Hant`, comma separated, default every shipped language), `q` matches names in the selected languages and codes, `limit` 1 to 500 (default 50); 400 on an unknown language |
| `GET /v1/wiki/{characters,skills,teams}/{key}?lang=` | 200 one record, 404 when no entry matches; key is the `chara_param` id (`0xXXXXXXXX`) or internal code, the `skillID` or code, the `belongTeamId` |
| `GET /wiki` | HTML home: languages, the tables behind each family and their entry counts, the families without a source |
| `GET /wiki/{lang}/{characters,skills,teams}?q=&limit=&offset=` | HTML list in the language `{lang}` (a BCP 47 tag), plain semantic HTML without script |
| `GET /wiki/{lang}/{characters,skills,teams}/{key}` | HTML page: values as stored, labels in every language, the VFS path, table and index behind each part, explicit empty states; team emblems are `<img src="/tex/<path>.png">` on the same host (`nie-model-serve`) |

The wiki routes decode their tables once per process, on first use (and in the background at start
once a VFS is mounted); without a VFS they answer 503. Their rules are in [wiki.md](wiki.md).

VFS paths are relative, `/`-separated, without `..`, empty segments, backslashes or a leading `/`
(for example `data/common/text/fr.cfg.bin`).

## `nie-model-serve` routes (CDN origin)

Derived routes are public; `/raw/`, `/depot/`, `/vfs/hash` and `/export/` in the `raw` format need
the bearer token and answer 404 without one. Paths after the prefix are VFS paths, with or
without the leading `data/`. Successful responses are `public, max-age=31536000, immutable`
(a Steam build id never changes a path's content; purge `NIE_CACHE_DIR` after a game update),
errors are `no-store`. Audio and video support `Range`.

| route | content |
|---|---|
| `GET /health` | liveness |
| `GET /tex/<path>.png`, `/tex/<container>.g4tx/<name>.png` | texture as `image/png` (largest, or the named texture of a container) |
| `GET /tex-info/<path>.g4tx` | texture metadata, JSON |
| `GET /cfg/<path>.cfg.bin.json`, `/typed/<path>.cfg.bin.json` | cfg.bin as generic JSON, or as typed game structures `{family, data}` |
| `GET /audio/<path>.{hca,adx,acb,awb}[?cue=N\|id=N]`, `/audio-info/<path>` | `audio/wav`, or the bank's cue list as JSON |
| `GET /video/<path>.usm[?track=audio\|info=1]`, `/video/catalog.json` | remuxed `video/mp4`, the soundtrack as WAV, the film's description, or the catalogue |
| `GET /model-full/<code>.glb`, `/model-chr/`, `/model-avatar/`, `/model-edit/`, `/model-map/`, `/model-tree/`, `/model-report/` | assembled models as `model/gltf-binary`, reports as JSON |
| `GET /avatar/catalog.json`, `/avatar/resolve.json`, `/avatar/layout/`, `/avatar/icon/` | avatar catalogue and icons |
| `GET /menu-render/<screen>.png`, `/menu-tree`, `/story-scene`, `/lip/`, `/ui/theme.json`, `/ui/font-metrics.json`, `/ui/text.png`, `/icons/index.json` | menu, UI and story renderings |
| `GET /vfs/ls?path=&limit=&offset=`, `/vfs/find?q=&ext=&limit=&offset=`, `/vfs/stat?path=`, `/vfs/stats` | paged listing, search and stat (`readable`, `formats`, `describe`); expose only behind a trusted network |
| `GET /export/<path>?format=<id>` | the file converted to `png`, `webp`, `json`, `wav`, `mp4`, `glb` and others (the list for a file is in `/vfs/stat` `formats`), as a download; `format=raw` (the default) is gated |
| `GET /raw/<path>` | decompressed bytes of the entry (Lua scripts, cfg, any file), gated |

Lua and other source-like entries are served through the gated `/raw/` or `/export/?format=raw`.
