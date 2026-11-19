// SPDX-License-Identifier: Apache-2.0

//! High-throughput pipelined Steam CDN downloader + streaming Google Drive pack sync.
//!
//! Replaces `steamroom-client`'s single-file sequential loop and untuned HTTP pool with:
//! - Multi-depot parallel planning + downloads (`JoinSet`, `max_depot_parallel`)
//! - HTTP/2 adaptive multiplexed `reqwest::Client` (`tcp_nodelay`, high idle pool per host)
//! - Global chunk `Semaphore(max_downloads)` across ALL files in a depot simultaneously
//! - Streaming 1 MiB buffered SHA-1 existing-file verification (zero giant RAM reads)
//! - Optional parallel `nie-rclone nie-export --mode decrypted-packs` pipeline per completed `.cpk`

use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use anyhow::Context;
use nie_formats::vfs_remote::{RcloneExportOptions, rclone_export_packs_with_opts};
use nie_steam::depot::depot_resolver::{proxied_from_app, read_manifest_gid, select_depot_ids};
use nie_steam::depot::options::SteamDownloadOptions;
use nie_steam::depot::planning::is_manifest_directory;
use nie_steam::depot::session::SteamSession;
use sha1::{Digest, Sha1};
use steamroom::cdn::{CdnServer, CdnServerPool, lancache};
use steamroom::depot::DepotKey;
use steamroom::depot::chunk::process_chunk;
use steamroom::depot::manifest::{DepotManifest, ManifestFile};
use tokio::sync::{Semaphore, mpsc};
use tokio::task::JoinSet;

const INVALID_MANIFEST: u64 = u64::MAX;
const CHUNK_RETRIES: usize = 5;

#[derive(Clone, Debug)]
pub struct TurboConfig {
    pub cdn_pool_size: usize,
    pub sync_remote: Option<String>,
    pub sync_mode: String,
    pub sync_transfers: usize,
    pub sync_checkers: usize,
}

pub struct TurboSummary {
    pub app_id: u32,
    pub depots: Vec<u32>,
    pub file_count: u64,
    pub downloaded_bytes: u64,
    pub skipped_files: u64,
    pub synced_batches: u64,
    pub duration: Duration,
}

struct DepotTaskPlan {
    depot_id: u32,
    manifest_id: u64,
    depot_key: DepotKey,
    request_code: u64,
    cdn_auth_token: Option<String>,
}

struct SharedCdnHttp {
    client: reqwest::Client,
    pool: Arc<CdnServerPool>,
    lancache_ip: Option<std::net::IpAddr>,
}

impl SharedCdnHttp {
    fn new(servers: Vec<CdnServer>, cdn_pool_size: usize) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .pool_max_idle_per_host(cdn_pool_size.max(64))
            .pool_idle_timeout(Duration::from_secs(90))
            .tcp_nodelay(true)
            .http2_adaptive_window(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(90))
            .user_agent("Valve/Steam HTTP Client 1.0")
            .build()
            .context("build tuned reqwest HTTP/2 CDN client")?;
        Ok(Self {
            client,
            pool: Arc::new(CdnServerPool::new(servers)),
            lancache_ip: lancache::detect(),
        })
    }

    async fn get_bytes(&self, path: &str, token: Option<&str>) -> anyhow::Result<Vec<u8>> {
        let mut last_err = anyhow::anyhow!("no CDN servers available");
        for attempt in 0..CHUNK_RETRIES {
            let (server, delay) = self.pool.pick();
            if !delay.is_zero() {
                tokio::time::sleep(delay.min(Duration::from_secs(5))).await;
            }
            let (url, host_hdr) = if let Some(ip) = self.lancache_ip {
                (
                    lancache::build_url(ip, server, path),
                    Some(lancache::host_header(server)),
                )
            } else {
                (server.build_url(path, token), None)
            };

            let mut req = self.client.get(&url);
            if let Some(h) = host_hdr {
                req = req.header(reqwest::header::HOST, h);
            }
            match req.send().await {
                Ok(resp) if resp.status().is_success() => {
                    let bytes = resp.bytes().await.context("read CDN response body")?;
                    self.pool.report_success(server);
                    return Ok(bytes.to_vec());
                }
                Ok(resp) => {
                    let code = resp.status();
                    let retry_after = resp
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                        .map(Duration::from_secs);
                    self.pool.report_failure(server, retry_after);
                    last_err = anyhow::anyhow!("HTTP {code} from {}", server.host);
                }
                Err(e) => {
                    self.pool.report_failure(server, None);
                    last_err = anyhow::anyhow!("CDN request failed ({}): {e}", server.host);
                }
            }
            let backoff = Duration::from_millis(100 * (1u64 << attempt.min(4)));
            tokio::time::sleep(backoff).await;
        }
        Err(last_err)
    }
}

fn decompress_manifest_if_zipped(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    if data.len() > 2 && data[0] == 0x50 && data[1] == 0x4B {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(data))
            .context("parse zipped Steam manifest")?;
        if archive.is_empty() {
            anyhow::bail!("empty Steam manifest zip");
        }
        let mut entry = archive.by_index(0)?;
        let mut out = Vec::new();
        entry.read_to_end(&mut out)?;
        Ok(out)
    } else {
        Ok(data.to_vec())
    }
}

fn stream_sha1_matches(path: &Path, expected_size: u64, expected_sha1: &[u8; 20]) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    if !meta.is_file() || meta.len() != expected_size {
        return false;
    }
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let mut hasher = Sha1::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        match f.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(_) => return false,
        }
    }
    let actual: [u8; 20] = hasher.finalize().into();
    &actual == expected_sha1
}

fn safe_relative_path(install_dir: &Path, normalized: &str) -> anyhow::Result<PathBuf> {
    let trimmed = normalized.trim_matches('/');
    if trimmed.is_empty() {
        return Ok(install_dir.to_path_buf());
    }
    let rel = Path::new(trimmed);
    for comp in rel.components() {
        if matches!(
            comp,
            std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_)
        ) {
            anyhow::bail!("unsafe manifest path rejected: {normalized}");
        }
    }
    Ok(install_dir.join(rel))
}

async fn run_cloud_sync_worker(
    mut rx: mpsc::UnboundedReceiver<String>,
    install_dir: PathBuf,
    cfg: TurboConfig,
) -> u64 {
    let Some(remote) = cfg.sync_remote.as_deref() else {
        return 0;
    };
    let mut batches = 0u64;
    let src_s = install_dir.to_string_lossy().to_string();
    loop {
        let Some(first) = rx.recv().await else {
            break;
        };
        let mut pending = vec![first];
        while pending.len() < 8 {
            match rx.try_recv() {
                Ok(next) => pending.push(next),
                Err(_) => break,
            }
        }
        let remote_owned = remote.to_owned();
        let mode_owned = cfg.sync_mode.clone();
        let src_owned = src_s.clone();
        let transfers = cfg.sync_transfers;
        let checkers = cfg.sync_checkers;
        for pack_name in pending {
            if !install_dir.join("cpk_list.cfg.bin").is_file()
                && !install_dir.join("data").join("cpk_list.cfg.bin").is_file()
            {
                break;
            }
            let r = tokio::task::spawn_blocking({
                let s = src_owned.clone();
                let d = remote_owned.clone();
                let m = mode_owned.clone();
                move || {
                    rclone_export_packs_with_opts(&RcloneExportOptions {
                        local_data_dir: &s,
                        dst_uri: &d,
                        mode: &m,
                        dry_run: false,
                        transfers,
                        checkers,
                        zstd_level: 4,
                        max_packs: 0,
                        filter: Some(&pack_name),
                        watch_secs: None,
                    })
                }
            })
            .await;
            if let Ok(Ok(_)) = r {
                batches += 1;
            }
        }
    }
    if install_dir.join("cpk_list.cfg.bin").is_file()
        || install_dir.join("data").join("cpk_list.cfg.bin").is_file()
    {
        let r = tokio::task::spawn_blocking({
            let s = src_s;
            let d = remote.to_owned();
            let m = cfg.sync_mode.clone();
            let t = cfg.sync_transfers;
            let c = cfg.sync_checkers;
            move || {
                rclone_export_packs_with_opts(&RcloneExportOptions {
                    local_data_dir: &s,
                    dst_uri: &d,
                    mode: &m,
                    dry_run: false,
                    transfers: t,
                    checkers: c,
                    zstd_level: 4,
                    max_packs: 0,
                    filter: None,
                    watch_secs: None,
                })
            }
        })
        .await;
        if let Ok(Ok(_)) = r {
            batches += 1;
        }
    }
    batches
}

/// Executes concurrent multi-depot, multi-file, pipelined Steam CDN acquisition + cloud sync.
pub async fn download_app_turbo(
    opts: &SteamDownloadOptions,
    cfg: &TurboConfig,
) -> anyhow::Result<TurboSummary> {
    let start = Instant::now();
    std::fs::create_dir_all(&opts.install_dir)
        .with_context(|| format!("create {}", opts.install_dir.display()))?;

    let mut session = SteamSession::connect(opts).await?;
    session.request_app_info(opts.app_id, false).await?;
    let depots_kv = session
        .app_kv(opts.app_id)
        .and_then(|kv| kv.get("depots"))
        .cloned()
        .context("missing depots section in PICS app info")?;

    let arch_filter = opts.arch.as_deref().filter(|s| !s.is_empty());
    let selected_depots = select_depot_ids(
        &depots_kv,
        &opts.depot_ids,
        opts.all_platforms,
        &opts.os,
        arch_filter,
        &opts.language,
    );
    if selected_depots.is_empty() {
        anyhow::bail!("no eligible depots for app {}", opts.app_id);
    }

    let servers = session.cdn_servers(0, Some(64)).await?;
    if servers.is_empty() {
        anyhow::bail!("no Steam CDN servers returned");
    }
    let primary_host = servers[0].host.clone();
    let http = Arc::new(SharedCdnHttp::new(servers, cfg.cdn_pool_size)?);

    let mut plans = Vec::new();
    for depot_id in selected_depots {
        let Some(depot_child) = depots_kv.get(&depot_id.to_string()) else {
            continue;
        };
        let proxy_app = proxied_from_app(depot_child);
        let manifest_id = if proxy_app != 0 && proxy_app != opts.app_id {
            session.request_app_info(proxy_app, false).await?;
            session
                .app_kv(proxy_app)
                .and_then(|kv| kv.get("depots"))
                .and_then(|d| d.get(&depot_id.to_string()))
                .map_or(INVALID_MANIFEST, |d| read_manifest_gid(d, &opts.branch))
        } else {
            read_manifest_gid(depot_child, &opts.branch)
        };
        if manifest_id == INVALID_MANIFEST {
            continue;
        }
        let Some(depot_key) = session.request_depot_key(depot_id, opts.app_id).await? else {
            continue;
        };
        let request_code = session
            .manifest_request_code(opts.app_id, depot_id, manifest_id, &opts.branch)
            .await;
        let cdn_auth_token = session
            .cdn_auth_token(opts.app_id, depot_id, &primary_host)
            .await;
        plans.push(DepotTaskPlan {
            depot_id,
            manifest_id,
            depot_key,
            request_code,
            cdn_auth_token,
        });
    }
    if plans.is_empty() {
        anyhow::bail!("no downloadable depots with valid keys/manifests");
    }

    let manifest_cache_dir = Arc::new(opts.install_dir.join(".nie-steam-cache").join("manifests"));
    let _ = std::fs::create_dir_all(manifest_cache_dir.as_ref());
    let chunk_sem = Arc::new(Semaphore::new(opts.max_downloads.max(16)));
    let depot_sem = Arc::new(Semaphore::new(opts.transfer.max_depot_parallel.max(2)));
    let verify_sem = Arc::new(Semaphore::new(
        std::thread::available_parallelism()
            .map_or(8, usize::from)
            .max(4),
    ));

    let (sync_tx, sync_handle) = if cfg.sync_remote.is_some() {
        let (tx, rx) = mpsc::unbounded_channel();
        let h = tokio::spawn(run_cloud_sync_worker(
            rx,
            opts.install_dir.clone(),
            cfg.clone(),
        ));
        (Some(tx), Some(h))
    } else {
        (None, None)
    };

    let total_downloaded = Arc::new(AtomicU64::new(0));
    let total_files = Arc::new(AtomicU64::new(0));
    let skipped_files = Arc::new(AtomicU64::new(0));
    let mut completed_depots = Vec::with_capacity(plans.len());

    let mut depot_join = JoinSet::new();
    for plan in plans {
        completed_depots.push(plan.depot_id);
        let http = http.clone();
        let chunk_sem = chunk_sem.clone();
        let depot_sem = depot_sem.clone();
        let verify_sem = verify_sem.clone();
        let manifest_cache_dir = manifest_cache_dir.clone();
        let install_dir = opts.install_dir.clone();
        let verify = opts.verify;
        let total_downloaded = total_downloaded.clone();
        let total_files = total_files.clone();
        let skipped_files = skipped_files.clone();
        let sync_tx = sync_tx.clone();

        depot_join.spawn(async move {
            let _depot_permit = depot_sem.acquire_owned().await?;
            let cache_file =
                manifest_cache_dir.join(format!("{}_{}.bin", plan.depot_id, plan.manifest_id));
            let raw_manifest = if let Ok(cached) = std::fs::read(&cache_file) {
                cached
            } else {
                let path = format!(
                    "/depot/{}/manifest/{}/5/{}",
                    plan.depot_id, plan.manifest_id, plan.request_code
                );
                let body = http
                    .get_bytes(&path, plan.cdn_auth_token.as_deref())
                    .await
                    .with_context(|| {
                        format!(
                            "fetch manifest {} depot {}",
                            plan.manifest_id, plan.depot_id
                        )
                    })?;
                let bytes = decompress_manifest_if_zipped(&body)?;
                let _ = std::fs::write(&cache_file, &bytes);
                bytes
            };

            let mut manifest = DepotManifest::parse(&raw_manifest)
                .with_context(|| format!("parse manifest {}", plan.manifest_id))?;
            if manifest.filenames_encrypted {
                manifest
                    .decrypt_filenames(&plan.depot_key)
                    .with_context(|| {
                        format!("decrypt manifest filenames depot {}", plan.depot_id)
                    })?;
            }

            // 1. Create directories and check existing files in parallel without full RAM reads.
            let mut created_dirs = HashSet::new();
            let mut verify_join = JoinSet::new();
            let mut files_to_download: Vec<(ManifestFile, PathBuf)> = Vec::new();

            for file in manifest.files {
                let norm = file.normalized_path();
                let target = safe_relative_path(&install_dir, &norm)?;
                if is_manifest_directory(file.flags) {
                    if created_dirs.insert(target.clone()) {
                        std::fs::create_dir_all(&target)?;
                    }
                    continue;
                }
                if let Some(parent) = target.parent()
                    && created_dirs.insert(parent.to_path_buf())
                {
                    std::fs::create_dir_all(parent)?;
                }
                if file.size == 0 && file.chunks.is_empty() {
                    if !target.exists() {
                        std::fs::File::create(&target)?;
                    }
                    total_files.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if verify && let Some(sha) = file.sha_content {
                    let vsem = verify_sem.clone();
                    verify_join.spawn(async move {
                        let _p = vsem.acquire_owned().await.ok();
                        let t = target.clone();
                        let size = file.size;
                        let matched = tokio::task::spawn_blocking(move || {
                            stream_sha1_matches(&t, size, &sha)
                        })
                        .await
                        .unwrap_or(false);
                        (file, target, matched)
                    });
                } else {
                    files_to_download.push((file, target));
                }
            }

            while let Some(res) = verify_join.join_next().await {
                let (file, target, matched) = res?;
                if matched {
                    skipped_files.fetch_add(1, Ordering::Relaxed);
                    total_files.fetch_add(1, Ordering::Relaxed);
                } else {
                    files_to_download.push((file, target));
                }
            }

            // Ensure cpk_list.cfg.bin arrives before large packs so continuous cloud export works on first pack.
            files_to_download.sort_by_key(|(f, _)| {
                let is_cfg = f.normalized_path().ends_with("cpk_list.cfg.bin");
                (!is_cfg, f.size)
            });

            // 2. Schedule ALL chunks across ALL remaining files into one global concurrent JoinSet.
            let mut chunk_join = JoinSet::new();
            for (file, target) in files_to_download {
                let chunk_count = file.chunks.len();
                let file_handle = tokio::task::spawn_blocking({
                    let t = target.clone();
                    let sz = file.size;
                    move || -> anyhow::Result<Arc<std::sync::Mutex<std::fs::File>>> {
                        let f = std::fs::OpenOptions::new()
                            .create(true)
                            .read(true)
                            .write(true)
                            .truncate(false)
                            .open(&t)
                            .with_context(|| format!("open {}", t.display()))?;
                        f.set_len(sz)?;
                        Ok(Arc::new(std::sync::Mutex::new(f)))
                    }
                })
                .await??;

                let remaining = Arc::new(AtomicUsize::new(chunk_count));
                let pack_sync_name = target
                    .file_name()
                    .and_then(|n| n.to_str())
                    .filter(|n| n.ends_with(".cpk") || n.eq_ignore_ascii_case("cpk_list.cfg.bin"))
                    .map(str::to_owned);

                for chunk in file.chunks {
                    let http = http.clone();
                    let chunk_sem = chunk_sem.clone();
                    let file_handle = file_handle.clone();
                    let depot_key = plan.depot_key.clone();
                    let token = plan.cdn_auth_token.clone();
                    let total_downloaded = total_downloaded.clone();
                    let total_files = total_files.clone();
                    let remaining = remaining.clone();
                    let sync_tx = sync_tx.clone();
                    let pack_sync_name = pack_sync_name.clone();
                    let depot_id = plan.depot_id;

                    chunk_join.spawn(async move {
                        let _permit = chunk_sem.acquire_owned().await?;
                        let path = format!("/depot/{depot_id}/chunk/{}", chunk.id);
                        let raw = http.get_bytes(&path, token.as_deref()).await?;
                        let uncompressed_len = chunk.uncompressed_size;
                        let checksum = chunk.checksum;
                        let offset = chunk.offset.unwrap_or(0);
                        let decoded = tokio::task::spawn_blocking(move || {
                            process_chunk(&raw, &depot_key, uncompressed_len, checksum)
                        })
                        .await?
                        .map_err(|e| anyhow::anyhow!("process chunk {}: {e}", chunk.id))?;

                        let written = decoded.len() as u64;
                        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                            let mut guard = file_handle
                                .lock()
                                .map_err(|_| anyhow::anyhow!("file mutex poisoned"))?;
                            guard.seek(SeekFrom::Start(offset))?;
                            guard.write_all(&decoded)?;
                            Ok(())
                        })
                        .await??;

                        total_downloaded.fetch_add(written, Ordering::Relaxed);
                        if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                            total_files.fetch_add(1, Ordering::Relaxed);
                            if let (Some(tx), Some(name)) = (sync_tx, pack_sync_name) {
                                let _ = tx.send(name);
                            }
                        }
                        Ok::<(), anyhow::Error>(())
                    });
                }
            }

            while let Some(res) = chunk_join.join_next().await {
                res??;
            }
            Ok::<(), anyhow::Error>(())
        });
    }

    while let Some(res) = depot_join.join_next().await {
        res??;
    }

    drop(sync_tx);
    let synced_batches = if let Some(h) = sync_handle {
        h.await.unwrap_or(0)
    } else {
        0
    };

    Ok(TurboSummary {
        app_id: opts.app_id,
        depots: completed_depots,
        file_count: total_files.load(Ordering::Relaxed),
        downloaded_bytes: total_downloaded.load(Ordering::Relaxed),
        skipped_files: skipped_files.load(Ordering::Relaxed),
        synced_batches,
        duration: start.elapsed(),
    })
}
