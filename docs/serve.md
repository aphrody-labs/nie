# Running nie as a service

Two HTTP servers ship with every release. Neither carries game data: both read the installation
the operator owns, in place, through the VFS (`nie-formats`).

| binary | what it is for |
|---|---|
| `nie serve` | JSON API: VFS listing and lookup, format detection and decoding, game-data queries, the wiki (JSON and HTML), `/openapi.json` |
| `nie-model-serve` | asset origin for a CDN: decoded textures (PNG), audio (WAV), video (MP4), cfg as JSON, models (GLB), raw bytes behind a token |

Release archives (`nie-v<version>-x86_64-unknown-linux-musl.tar.gz`,
`nie-v<version>-x86_64-pc-windows-msvc.zip`) hold the full `host` build of `nie` and
`nie-model-serve` (and `nie-deploy` on Linux), plus `SHA256SUMS` on the release page.

## Configuration

Environment variables (a flag of the same name wins). The defaults bind `127.0.0.1:8790`.

| variable | default | meaning |
|---|---|---|
| `NIE_HOST` | `127.0.0.1` | bind address of `nie serve` (`NIE_MODEL_SERVE_HOST` for `nie-model-serve`) |
| `NIE_PORT` | `8790` | TCP port, read by both servers |
| `NIE_GAME_SOURCE` | discovery | where the installation lives, see below; `NIE_GAME_DIR` is also read |
| `NIE_RAW_VFS_TOKEN` | unset | bearer token of at least 32 bytes that opens the game's untouched bytes; unset or shorter keeps them closed |
| `NIE_HTTP_AUTH_TOKEN` | unset | bearer sent to an `https://` source |
| `NIE_CACHE_DIR` | `<game>/var/model-cache` | `nie-model-serve` cache of assembled GLB, mixed WAV and MP4 |
| `RCLONE_CONFIG` | `~/.config/rclone/rclone.conf` | rclone configuration, parsed in Rust, for `r2://`, `s3://`, `gdrive://`, `rclone://` sources; `RCLONE_CONFIG_<REMOTE>_<KEY>` variables also define remotes |

`nie serve` flags: `--strict` exits at startup when the source cannot be mounted; `--stateless`
makes `/ready` healthy without a VFS (format and registry worker mode).

### Game source (`NIE_GAME_SOURCE`)

| value | transport |
|---|---|
| `/game`, `D:\Games\ievr` | local files, CPK ranges read in place |
| `ssh://[user@]host[:port]/path`, `sftp://...` | SFTP over `russh`, no `ssh` binary: host keys from `~/.ssh/known_hosts`, identities from `ssh-agent` and `~/.ssh` keys, `~/.ssh/config` (`HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`, `ConnectTimeout`) |
| `r2://remote/bucket/path`, `s3://`, `gdrive://remote/path`, `rclone://remote/path` | `opendal` ranged reads, no `rclone` binary; remotes of type `s3` (R2, AWS, others), `drive`, `sftp`, `http` and `alias` from `rclone.conf`; `crypt` and other types are reported unsupported |
| `https://host/path` | HTTP range requests (`opendal`); `http://` uses the built-in client |

Credentials never go in the URI. Only the CPK tables and the requested ranges are transferred.
No `ssh` or `rclone` binary is needed: every transport is Rust.

### Production host

No init system units and no container runtime: `nie-deploy supervise` (crates/nie-deploy) runs
the stack. Each service gets its own session (`setsid`), user, rlimits and `no_new_privs`; the
supervisor owns the process tree (a stop kills the whole group), restarts with exponential backoff
(1 s doubling to 60 s, reset after a stable minute), rotates one log per service under
`/var/log/nie` by size (10 MiB, 5 kept), and restarts a service whose HTTP probe fails four times
in a row. One instance per host (flock on `/run/nie/supervisor.lock`); its state is
`/run/nie/status.json`. SIGTERM stops everything, SIGHUP restarts every service.

`deploy/supervise.toml` (embedded in the binary) describes the host: `nie serve` on
`127.0.0.1:8796` and `nie-model-serve` on `127.0.0.1:8797` as user `nie`, and the Cloudflare
tunnel connector for the public hostname (prd's 443 belongs to the shared nginx; the `nie-edge`
TLS listener on a Cloudflare origin port is the `nie-deploy cloudflare` path). `/srv/nie/steam` is
the Steam library of the game: `steamapps/common/INAZUMA ELEVEN Victory Road` and
`steamapps/appmanifest_2799860.acf`. Steam tooling state (manifest cache, depot downloader) lives in
`/var/lib/nie/steam`, never inside the game directory.

```
nie-deploy up --host prd --bin-dir <dir with nie-deploy [nie nie-model-serve]>   # over nie-ssh
nie-deploy status --host prd --json                                                # supervisor, services, live probes
nie-deploy up                                                                      # on the host itself (root)
```

`up` uploads the binaries of `--bin-dir` whose SHA-256 differs (atomic rename), installs
`/etc/nie/supervise.toml`, installs the start at boot (`/etc/cron.d/nie-deploy`, a one-minute `supervise --detach` watchdog that is a
no-op while the supervisor runs; else a managed block in `/etc/rc.local` where the rc-local generator
exists; `status` reports `boot_hook`), starts the supervisor detached when it is not running, restarts it when its
configuration or binary changed, restarts the services when their binaries changed, and waits until
every probe passes. Re-running it on an unchanged host changes nothing.

Remote sources: `rclone.conf` at `~nie/.config/rclone/rclone.conf` and `~nie/.ssh` readable by
user `nie`. The environment of a service is the `env` and `env_files` of its entry, nothing is
inherited from the supervisor.

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
| `GET /v1/vfs/remotes` | 200 `{remotes:[{name, type, supported, reason?}]}`, the remotes of `rclone.conf` (empty without one); 503 when the file cannot be read |
| `POST /v1/formats/detect` | body = bytes; 200 `{format, size}` |
| `POST /v1/formats/decode?name=&format=` | body = bytes; `format=png` returns `image/png` (G4TX), else the decoded structure as JSON; 422 when unreadable |
| `GET /v1/vfs/info` | 200 `{source, entries, cpks, transport:{ioCalls, payloadBytes}}`; 503 without VFS |
| `GET /v1/vfs/ls?path=&limit=&offset=` | 200 `{dir, dirs:[{name, files}], files:[{name, ext, path, size, cpk}], fileTotal, fileOffset}`: one folder of the VFS, exactly as `data/common` and `data/dx11` hold it; `limit` 1 to 5000 (default 1000); 404 for a folder with no entry, 400 invalid path |
| `GET /v1/vfs/find?q=&ext=&limit=` | 200 `{count, items:[{path, size, cpk}]}`, `limit` 1 to 500 (default 50) |
| `GET /v1/vfs/stat?path=` | 200 `{path, size, cpk}`; 400 invalid path, 404 unknown |
| `GET /v1/vfs/read?path=&decode=` | `decode=json`: decoded structure (`application/json`, `max-age=300`); `decode=png`: texture as `image/png`; otherwise the raw bytes as `application/octet-stream`, which needs `Authorization: Bearer <NIE_RAW_VFS_TOKEN>` and is a 404 for everyone while no token is configured; 422 when not decodable |
| `GET /v1/game/query?domain=&q=&limit=` | 200 game-data rows of `nie_explore::domain_data::query_vfs` (default domain `characters`), 400 on an unknown domain |
| `GET /openapi.json` | the OpenAPI 3.1 description of every route in this table, generated from the same route list the router dispatches on (`crates/nie-cli/src/serve_routes.rs`; a test compares the two) |
| `GET /v1/wiki/languages` | 200 `{count, items:[{folder, tag}]}`: the folders of `data/common/text` that hold the character names, with their BCP 47 tags |
| `GET /v1/wiki/unsourced` | 200 `{count, items:[{family, reason}]}`: the families the VFS gives no source for |
| `GET /v1/wiki/{characters,skills,teams}?lang=&q=&limit=&offset=` | 200 `{total, offset, limit, items}`; `lang` takes folders or BCP 47 tags (`de en es fr it ja pt zh-Hans zh-Hant`, comma separated, default every shipped language), `q` matches names in the selected languages and codes, `limit` 1 to 500 (default 50); 400 on an unknown language. Characters also take `class=param_and_base\|param_without_base\|base_without_param` and carry `classes`, the record count of each class: every `chara_param` and every `chara_base` entry is one record |
| `GET /v1/wiki/{characters,skills,teams}/{key}?lang=` | 200 one record, 404 when no entry matches; key is the `chara_param` id (`0xXXXXXXXX`), `base-<n>` (entry n of `chara_base`), the base id or the internal code; the `skillID` or code; the `belongTeamId` |
| `GET /v1/wiki/tables` | 200 `{count, items:[{table, vfs_path, format, classes:[{name, role, entries, target?, rich_family?}]}]}`: every decoded table (`chara_param`, `chara_base`, `chara_series_config`, `seasonal_chara_param`, `chara_param_table_config`, `skill_config`, `passive_skill_config`, `aura_skill_config`, `change_aura_skill_config`, `override_skill_config`, `real_skill_config`, `belong_team_config`) and each node class or list |
| `GET /v1/wiki/tables/{table}/{class}?lang=&q=&limit=&offset=` | 200 `{total, offset, limit, items}` of the class's entries as stored (`fields` of an RDBN row, `variables` of a T2B node, its `sort_index`, its `refs` slices, the `labels` of its text references); 404 unknown table or class |
| `GET /v1/wiki/tables/{table}/{class}/{index}?lang=` | 200 one entry, 404 when the index is outside the class, 400 when it is not a number |
| `GET /v1/wiki/catalog` | 200 `{count, items:[{vfs_path, status}]}`: every file of `data/common/gamedata`, `decoded` when a table above reads it, `present` otherwise |
| `GET /wiki` | HTML home: languages, the tables behind each family and their entry counts, the character classes, the families without a source, every gamedata file with its status |
| `GET /wiki/{lang}/{characters,skills,teams}?q=&limit=&offset=` | HTML list in the language `{lang}` (a BCP 47 tag), plain semantic HTML without script; characters also take `class=` |
| `GET /wiki/{lang}/{characters,skills,teams}/{key}` | HTML page: values as stored, labels in every language, the VFS path, table and index behind each part, explicit empty states; team emblems are `<img src="/tex/<path>.png">` on the same host (`nie-model-serve`) |
| `GET /wiki/{lang}/tables`, `/wiki/{lang}/tables/{table}/{class}`, `/wiki/{lang}/tables/{table}/{class}/{index}` | HTML pages of the table classes and their entries as stored |

The wiki routes decode their tables once per process, on first use (and in the background at start
once a VFS is mounted); without a VFS they answer 503. Their rules are in [wiki.md](wiki.md).

VFS paths are relative, `/`-separated, without `..`, empty segments, backslashes or a leading `/`
(for example `data/common/text/fr.cfg.bin`).

## `nie-model-serve` routes (CDN origin)

Derived routes are public; `/raw/`, `/vfs/hash` and `/export/` in the `raw` format need
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
| `GET /video/<path>.usm[?track=audio\|info=1]`, `/video/catalog.json` | remuxed `video/mp4`, the soundtrack as WAV, the film's description, or the catalogue of `data/common/movie`; the catalogue is built from the VFS in the background and answers 503 `Retry-After` until ready; a film is served only from the path asked for, 404 when the VFS has no such entry |
| `GET /model-full/<code>.glb`, `/model-chr/`, `/model-avatar/`, `/model-edit/`, `/model-map/`, `/model-tree/`, `/model-report/` | assembled models as `model/gltf-binary`, reports as JSON |
| `GET /avatar/icon/<name>.png` | an avatar editor thumbnail, decoded from its atlas |
| `GET /menu-tree[.json]`, `/menu-tree/<screen>[.json]` | every menu screen (layers, resources, commands) from the `data/common/gamedata/menu/cfg/*_setting.cfg.bin` files |
| `GET /lip/`, `/ui/theme.json`, `/ui/font-metrics.json`, `/ui/text.png`, `/icons/index.json` | lip-sync visemes, the FONT_COLOR palette and font families, the font metrics, a text in the game font, and the icon index computed from the menu atlases |
| `GET /vfs/ls?path=&limit=&offset=`, `/vfs/find?q=&ext=&limit=&offset=`, `/vfs/stat?path=`, `/vfs/stats` | paged listing, search and stat (`readable`, `formats`, `describe`); expose only behind a trusted network |
| `GET /export/<path>?format=<id>` | the file converted to `png`, `webp`, `json`, `wav`, `mp4`, `glb` and others (the list for a file is in `/vfs/stat` `formats`), as a download; `format=raw` (the default) is gated |
| `GET /raw/<path>` | decompressed bytes of the entry (Lua scripts, cfg, any file), gated |

Lua and other source-like entries are served through the gated `/raw/` or `/export/?format=raw`.

Every route reads the VFS and nothing else: the CRC32 table of the mesh files, the outfit recipes
and the character cards are computed from the VFS at start, the icon index and the video
catalogue on first use. `GET /openapi.json` is generated from the route table
(`crates/nie-model-serve/src/routes.rs`); `contract/openapi.json` is its checked-in output
(`nie-model-serve --print-openapi`) and a test fails when they drift, and another when the
handler serves a path the table does not list or the table lists one the handler does not
serve.

## Self-hosting: one static binary and the browser app

`nie serve` is the whole product on a host: the JSON API, the wiki, `/openapi.json` and the browser
app. The app (the `nie-wasm` module with its WebGPU viewer, its glue and its page) is compiled into
the binary by `crates/nie-app`, so the server and the client ship as one file. No Docker, no Node,
no CDN and no Cloudflare is needed; `nie-edge` stays an optional front for TLS and authenticated
origin pulls.

Three steps on any Linux host:

1. **Binary.** Take `nie` from a release archive, or build it with the app embedded. The app is
   built first, then embedded, on the build host or in CI:

   ```
   cargo run --release --manifest-path tools/nie-app-pack/Cargo.toml -- --repo . --out dist/app
   NIE_APP_DIST=$PWD/dist/app cargo build --locked --release --target x86_64-unknown-linux-musl \
       --no-default-features -p nie-cli --features nie-cli/cloud --bin nie
   ```

   `nie-app-pack` runs `cargo build -p nie-wasm --target wasm32-unknown-unknown`, `wasm-bindgen`
   at the version `Cargo.lock` pins (it refuses another), `wasm-opt -Oz`, then renames every file
   with the first eight hex digits of its BLAKE3 and writes a brotli copy of each file it shrinks.
   A binary built without `NIE_APP_DIST` serves no app: `/app` answers 404 with the reason.
2. **Game path.** `NIE_GAME_SOURCE` names the installation the operator owns (see the table above).
   Set `NIE_RAW_VFS_TOKEN` (32 bytes or more) to let a browser read the packs through
   `/v1/vfs/stream`; without it that route answers 404 and the app can only use a folder the user
   picks on their own machine.
3. **Unit.** A systemd service with `RequiresMountsFor=` on the game directory, a dedicated user and
   state under `/var/lib/nie`; `deploy/systemd/nie-serve.service` is the reference. The service
   needs read access to the game and nothing else.

### The app

`GET /app/` is the page. Every file but the page carries its content hash in its name and is served
`public, max-age=31536000, immutable`; the page itself is `no-cache`, so a new build is picked up on
the next load. A browser asking for `/` (`Accept: text/html`) is redirected to `/app/`; every other
client still gets the health JSON there.

The page reads the installation in place, in one of two ways:

| mode | transport | what leaves the machine |
|---|---|---|
| **Open the game folder** | the File System Access API on the folder the user picks; the page reads byte ranges of the packs, and the handle is kept in IndexedDB | nothing: no hosted game data is needed at all |
| **Connect** | HTTP `Range` requests to `/v1/vfs/stream` of the user's own `nie serve`, with `Authorization: Bearer <NIE_RAW_VFS_TOKEN>` | the requested ranges, from the user's server to the user's browser |

In both modes the page decrypts `cpk_list.cfg.bin`, reads the pack tables and decompresses entries
with the same Rust code as the native VFS (`nie_formats::source_vfs::SourceVfs` over the
`ByteSource` and `SourceTree` traits), then runs the explorer, the decoders, the wiki
(`nie_wiki_vfs`, computed in the tab) and the model viewer (`nie-render3d` on WebGPU, WebGL 2 where
WebGPU is missing). Browsers that lack the File System Access API (Firefox, Safari) use the server
mode. WebGPU and the folder picker need a secure context: `https://` or `http://localhost`.

### `GET /v1/vfs/stream?path=`

Byte ranges of the installation's own files, relative to its data folder: `cpk_list.cfg.bin`,
`packs/<pack>.cpk`, or a loose file the index declares. Nothing else on the disk is reachable.
These bytes are the game itself, so the route follows the raw-bytes rule of `/v1/vfs/read`: 404 for
everyone while no `NIE_RAW_VFS_TOKEN` is configured, then bearer token only. `Range: bytes=a-b`
answers 206 with `Content-Range: bytes a-b/total` and `Accept-Ranges: bytes`; a range past the end
answers 416; one answer carries at most 128 MiB. A mount over SSH or an rclone remote answers 404
(there is no local file to range over): serve from the host that holds the game.

### Browser check

`tools/nie-app-pack/browser-check.ts` (Bun) drives headless Chrome against a running server and
fails unless: the page loads, `navigator.gpu` exists, the console has no error, and the same files
mounted over HTTP `Range` and over a directory handle (the origin-private file system holding a
copy) give the BLAKE3 of the native VFS for every file. It takes a synthetic installation written
by `cargo run -p nie-wiki-vfs --features testing --example synth_install -- <dir> 50`; no game byte
is involved. The run on the build host and its screenshots are recorded in
`docs/research/log/2026-11-19-wasm-app.md`.
