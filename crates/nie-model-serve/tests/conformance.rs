//! Conformance of a running `nie-model-serve` against the VFS it serves.
//!
//! For a sample of every format family the server answers (G4TX textures, cfg.bin tables, audio,
//! video, assembled models), the test fetches the route over HTTP and compares the body with the
//! decode of the same VFS entry by the shared libraries, run locally on the same installation.
//! Derived deterministic outputs (PNG, JSON, WAV, MP4) must be equal byte for byte; absent
//! entries must answer 404 exactly.
//!
//! Skipped unless both variables are set:
//!
//! - `NIE_CONFORMANCE_BASE`: origin of the server, for example `https://nie.aphrody.app` or
//!   `http://127.0.0.1:8790`;
//! - `NIE_GAME_SOURCE`: the installation (local path, `ssh://`, `r2://`, ...), the same one the
//!   server mounts.
//!
//! `NIE_CONFORMANCE_SAMPLE` sets the sample size per family (default 6).

use std::collections::BTreeMap;
use std::time::Duration;

use nie_formats::cfgbin;
use nie_formats::g4tx_decode;
use nie_formats::vfs::Vfs;
use nie_formats::vfs_manifest::hex;
use nie_model_serve::decode::cfgbin_to_json;
use sha2::{Digest, Sha256};

struct Check {
    base: String,
    client: reqwest::blocking::Client,
    mismatches: Vec<String>,
    compared: BTreeMap<&'static str, usize>,
}

impl Check {
    fn get(&self, path: &str) -> Result<(u16, Vec<u8>), String> {
        let response = self
            .client
            .get(format!("{}{path}", self.base))
            .send()
            .map_err(|e| format!("{path}: {e}"))?;
        let status = response.status().as_u16();
        let body = response
            .bytes()
            .map_err(|e| format!("{path}: {e}"))?
            .to_vec();
        Ok((status, body))
    }

    fn fail(&mut self, family: &'static str, what: String) {
        self.mismatches.push(format!("{family}: {what}"));
    }

    fn compared(&mut self, family: &'static str) {
        *self.compared.entry(family).or_default() += 1;
    }

    /// The route must answer `expected` bytes (`Some`) or a 404 (`None`).
    fn expect(&mut self, family: &'static str, route: &str, expected: Option<Vec<u8>>) {
        self.compared(family);
        match (self.get(route), expected) {
            (Err(e), _) => self.fail(family, e),
            (Ok((200, body)), Some(local)) => {
                if body != local {
                    self.fail(
                        family,
                        format!(
                            "{route}: {} bytes served, {} bytes decoded locally (sha256 {} / {})",
                            body.len(),
                            local.len(),
                            hex(&Sha256::digest(&body)),
                            hex(&Sha256::digest(&local))
                        ),
                    );
                }
            }
            (Ok((404, _)), None) => {}
            (Ok((status, _)), Some(_)) => {
                self.fail(family, format!("{route}: status {status}, the VFS decodes it"));
            }
            (Ok((status, _)), None) => self.fail(
                family,
                format!("{route}: status {status}, the local decoder has no output (expected 404)"),
            ),
        }
    }
}

/// `count` paths spread evenly over the sorted candidates.
fn spread(mut paths: Vec<String>, count: usize) -> Vec<String> {
    paths.sort();
    if paths.len() <= count {
        return paths;
    }
    (0..count)
        .map(|i| paths[i * (paths.len() - 1) / (count - 1).max(1)].clone())
        .collect()
}

fn with_ext(vfs: &Vfs, ext: &str, under: &str) -> Vec<String> {
    vfs.iter()
        .map(|(p, _)| p)
        .filter(|p| p.starts_with(under) && p.ends_with(ext))
        .map(str::to_owned)
        .collect()
}

fn rel(path: &str) -> &str {
    path.strip_prefix("data/").unwrap_or(path)
}

#[test]
fn served_outputs_equal_the_local_decode_of_the_vfs() {
    let (Ok(base), Ok(source)) = (
        std::env::var("NIE_CONFORMANCE_BASE"),
        std::env::var("NIE_GAME_SOURCE"),
    ) else {
        eprintln!("skip: NIE_CONFORMANCE_BASE and NIE_GAME_SOURCE are not both set");
        return;
    };
    let sample: usize = std::env::var("NIE_CONFORMANCE_SAMPLE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6)
        .max(2);
    let vfs = nie_formats::vfs::open_source(Some(&source)).expect("mount the installation");
    let mut check = Check {
        base: base.trim_end_matches('/').to_owned(),
        client: reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(600))
            .build()
            .unwrap(),
        mismatches: Vec::new(),
        compared: BTreeMap::new(),
    };

    // G4TX: the default PNG of the container, and one named texture of some containers.
    for path in spread(with_ext(&vfs, ".g4tx", "data/dx11/"), sample * 2) {
        let bytes = vfs.read(&path).expect("read g4tx");
        let png = g4tx_decode::decode_best_to_png(&bytes, g4tx_decode::basename_of(&path));
        let route = format!("/tex/{}.png", rel(&path).trim_end_matches(".g4tx"));
        check.expect("g4tx png", &route, png);
        if let Ok(container) = nie_formats::g4tx::parse(&bytes)
            && let Some(texture) = container.textures.iter().find(|t| !t.name.is_empty())
        {
            let named = g4tx_decode::decode_named_to_png(&bytes, &texture.name);
            let route = format!("/tex/{}/{}.png", rel(&path), texture.name);
            check.expect("g4tx named png", &route, named);
        }
    }

    // cfg.bin: the generic JSON and the typed structure, from small tables.
    let small: Vec<String> = with_ext(&vfs, ".cfg.bin", "data/common/")
        .into_iter()
        .filter(|p| vfs.find(p).is_some_and(|e| e.file_size < 200_000))
        .collect();
    for path in spread(small, sample * 2) {
        let bytes = vfs.read(&path).expect("read cfg.bin");
        let generic = cfgbin_to_json(&bytes).and_then(|v| serde_json::to_vec(&v).ok());
        check.expect("cfg json", &format!("/cfg/{}.json", rel(&path)), generic);
        let typed = cfgbin::to_tagged_json(&bytes).and_then(|root| {
            let key = nie_data::typed::family_key(&path);
            let out = match nie_data::typed::decode_by_key(&key, &root) {
                Some((family, data)) => serde_json::json!({ "family": family, "data": data }),
                None => serde_json::json!({ "family": serde_json::Value::Null, "key": key, "generic": root }),
            };
            serde_json::to_vec(&out).ok()
        });
        check.expect("typed json", &format!("/typed/{}.json", rel(&path)), typed);
    }

    // Audio: banks with an embedded or a sibling AWB, decoded to WAV by the shared decoder.
    for path in spread(with_ext(&vfs, ".acb", "data/"), sample) {
        let bytes = vfs.read(&path).expect("read acb");
        let wav = nie_formats::cri_audio::decode_to_wav(&bytes).or_else(|e| {
            if e.to_string().contains("ACB sans AWB") {
                let awb = vfs
                    .read(&format!("{}.awb", path.trim_end_matches(".acb")))
                    .map_err(|_| e.to_string())?;
                nie_formats::cri_audio::decode_awb_entry(&awb, None).map_err(|e| e.to_string())
            } else {
                Err(e.to_string())
            }
        });
        // A bank the decoder refuses is answered with an error, never a 404: compare 200 only.
        if let Ok(wav) = wav {
            check.expect("audio wav", &format!("/audio/{}", rel(&path)), Some(wav));
        }
    }

    // Video: the smallest films, remuxed by the shared container code.
    let mut films: Vec<(u32, String)> = vfs
        .iter()
        .filter(|(p, _)| p.starts_with("data/common/movie/") && p.ends_with(".usm"))
        .map(|(p, e)| (e.file_size, p.to_owned()))
        .collect();
    films.sort();
    for (_, path) in films.into_iter().take(2) {
        let bytes = vfs.read(&path).expect("read usm");
        let name = path.rsplit('/').next().unwrap();
        if let Ok(video) = nie_explore::cinema::browser_video_container(&bytes, name) {
            check.expect("video", &format!("/video/{}", rel(&path)), Some(video.bytes));
        }
    }

    // Models: deterministic, a valid GLB, and a report whose digest and sources are the served
    // bytes and real VFS entries.
    let mut codes: Vec<String> = vfs
        .iter()
        .filter_map(|(p, _)| {
            let stem = p.rsplit('/').next()?.strip_suffix(".g4md")?;
            (stem.len() == 9
                && stem.starts_with('c')
                && stem[1..].bytes().all(|b| b.is_ascii_digit())
                && p.contains("/chr/"))
            .then(|| stem.to_owned())
        })
        .collect();
    codes.sort();
    codes.dedup();
    for code in spread(codes, 3) {
        check.compared("model");
        let (first, second) = (
            check.get(&format!("/model-full/{code}.glb")),
            check.get(&format!("/model-full/{code}.glb")),
        );
        let (Ok((200, glb)), Ok((200, again))) = (first, second) else {
            check.fail("model", format!("/model-full/{code}.glb is not served"));
            continue;
        };
        if glb != again {
            check.fail("model", format!("{code}: two requests gave different GLB bytes"));
        }
        if glb.get(..4) != Some(b"glTF") {
            check.fail("model", format!("{code}: not a GLB"));
        }
        match check.get(&format!("/model-report/{code}.json")) {
            Ok((200, body)) => {
                let report: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
                if report["glb_sha256"] != hex(&Sha256::digest(&glb)) {
                    check.fail("model", format!("{code}: the report digest is not the served GLB's"));
                }
                for piece in report["pieces"].as_array().into_iter().flatten() {
                    for key in ["g4md_path", "source"] {
                        if let Some(p) = piece[key].as_str().filter(|p| p.starts_with("data/"))
                            && vfs.find_any(p).is_none()
                        {
                            check.fail("model", format!("{code}: report names {p}, not in the VFS"));
                        }
                    }
                }
            }
            other => check.fail("model", format!("{code}: report answered {other:?}")),
        }
    }

    // Absent entries answer 404 exactly.
    for route in [
        "/tex/dx11/no/such/file.png",
        "/cfg/common/no/such/table.cfg.bin.json",
        "/typed/common/no/such/table.cfg.bin.json",
        "/audio/common/no/such/bank.acb",
        "/video/common/movie/no_such_film.usm",
        "/lip/dx11/no/such/track.p3lip.json",
    ] {
        check.expect("absent", route, None);
    }

    eprintln!("compared per family: {:?}", check.compared);
    assert!(
        check.mismatches.is_empty(),
        "{} mismatch(es):\n{}",
        check.mismatches.len(),
        check.mismatches.join("\n")
    );
}
