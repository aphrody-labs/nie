//! A content-addressed cache of decoded preview pictures.
//!
//! Decoding a BC7 texture or demuxing a film costs far more than reading a small PNG, so the
//! first preview of an entry is stored under the BLAKE3 hash of what identifies it: the logical
//! path, the archive, the size, the picture index and the decoder version. A new decoder version
//! changes every key, which retires the old entries without a migration. The design follows
//! spacedrive's `pvcache` (a content-addressed thumbnail store, Apache-2.0); no code is shared.
//!
//! The cache lives in the user cache directory and never inside the game directory: [`ThumbCache::open`]
//! refuses a location under the game data.

use std::fs;
use std::io::{self, Cursor, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::preview::Image;

/// Bump when a decoder changes what it produces: every key changes with it.
pub const DECODER_VERSION: u32 = 1;

/// Default size budget.
pub const DEFAULT_BUDGET_MIB: u64 = 256;

/// Environment variable that moves the cache.
pub const DIR_ENV: &str = "NIE_FM_CACHE_DIR";
/// Environment variable that sets the budget in MiB.
pub const BUDGET_ENV: &str = "NIE_FM_CACHE_MIB";

const MAGIC: &[u8; 4] = b"NFM1";

/// The identity of one cached picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Key([u8; 32]);

impl Key {
    /// Hashes the identity of a preview picture.
    #[must_use]
    pub fn new(path: &str, cpk: &str, size: u64, index: i32) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&DECODER_VERSION.to_le_bytes());
        hasher.update(&super::preview::MAX_SIDE.to_le_bytes());
        hasher.update(&size.to_le_bytes());
        hasher.update(&index.to_le_bytes());
        for part in [path, cpk] {
            hasher.update(&(part.len() as u64).to_le_bytes());
            hasher.update(part.as_bytes());
        }
        Self(*hasher.finalize().as_bytes())
    }

    /// The lower-case hex form, used as the file name.
    #[must_use]
    pub fn hex(&self) -> String {
        blake3::Hash::from_bytes(self.0).to_hex().to_string()
    }
}

/// What the cache stores for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cached {
    /// The previewer kind (`texture`, `image`).
    pub kind: String,
    /// The facts shown beside the picture.
    pub meta: Vec<(String, String)>,
    /// The picture.
    pub image: Image,
}

#[derive(Serialize, Deserialize)]
struct Header {
    kind: String,
    meta: Vec<(String, String)>,
}

/// The cache directory.
#[derive(Debug, Clone)]
pub struct ThumbCache {
    dir: PathBuf,
    budget: u64,
}

impl ThumbCache {
    /// Opens the default cache: `$NIE_FM_CACHE_DIR`, else `<user cache dir>/nie/fm`. `avoid` is the
    /// game data directory; a cache location inside it is refused.
    ///
    /// # Errors
    /// Fails when no cache directory exists for the user, when the location is inside `avoid`, or
    /// when it cannot be created.
    pub fn open(avoid: Option<&Path>) -> io::Result<Self> {
        let dir = match std::env::var_os(DIR_ENV).filter(|v| !v.is_empty()) {
            Some(dir) => PathBuf::from(dir),
            None => dirs::cache_dir()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no user cache directory"))?
                .join("nie")
                .join("fm"),
        };
        let budget = std::env::var(BUDGET_ENV)
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .unwrap_or(DEFAULT_BUDGET_MIB)
            * 1024
            * 1024;
        Self::open_in(dir, budget, avoid)
    }

    /// Opens a cache in `dir`.
    ///
    /// # Errors
    /// Fails when `dir` is inside `avoid` or cannot be created.
    pub fn open_in(dir: PathBuf, budget: u64, avoid: Option<&Path>) -> io::Result<Self> {
        fs::create_dir_all(&dir)?;
        let real = dir.canonicalize()?;
        if let Some(game) = avoid.and_then(|a| a.canonicalize().ok())
            && real.starts_with(&game)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "the thumbnail cache {} is inside the game directory {}",
                    real.display(),
                    game.display()
                ),
            ));
        }
        Ok(Self { dir: real, budget })
    }

    /// The directory of the cache.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, key: &Key) -> PathBuf {
        let hex = key.hex();
        self.dir.join(&hex[..2]).join(format!("{hex}.nfm"))
    }

    /// Looks an entry up. A damaged file is deleted and reported as a miss.
    #[must_use]
    pub fn get(&self, key: &Key) -> Option<Cached> {
        let path = self.file(key);
        let bytes = fs::read(&path).ok()?;
        match decode(&bytes) {
            Some(cached) => Some(cached),
            None => {
                let _ = fs::remove_file(path);
                None
            }
        }
    }

    /// Stores an entry; the file appears atomically.
    ///
    /// # Errors
    /// Fails when the file cannot be written.
    pub fn put(&self, key: &Key, cached: &Cached) -> io::Result<()> {
        let path = self.file(key);
        let parent = path.parent().unwrap_or(&self.dir);
        fs::create_dir_all(parent)?;
        let bytes = encode(cached)?;
        let tmp = parent.join(format!(".{}.{}.tmp", key.hex(), std::process::id()));
        {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(&bytes)?;
        }
        fs::rename(tmp, path)
    }

    /// Deletes the oldest entries until the cache is below 90 % of its budget. Returns the number
    /// of bytes left.
    pub fn prune(&self) -> u64 {
        let mut files: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let mut total = 0u64;
        let Ok(shards) = fs::read_dir(&self.dir) else {
            return 0;
        };
        for shard in shards.flatten() {
            let Ok(entries) = fs::read_dir(shard.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(meta) = entry.metadata() else { continue };
                if !meta.is_file() {
                    continue;
                }
                total += meta.len();
                files.push((
                    meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    meta.len(),
                    entry.path(),
                ));
            }
        }
        if total <= self.budget {
            return total;
        }
        files.sort_by_key(|(modified, _, _)| *modified);
        let target = self.budget / 10 * 9;
        for (_, len, path) in files {
            if total <= target {
                break;
            }
            if fs::remove_file(path).is_ok() {
                total -= len;
            }
        }
        total
    }
}

fn encode(cached: &Cached) -> io::Result<Vec<u8>> {
    if !cached.image.is_valid() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid image"));
    }
    let header = serde_json::to_vec(&Header {
        kind: cached.kind.clone(),
        meta: cached.meta.clone(),
    })
    .map_err(io::Error::other)?;
    let mut out = Vec::with_capacity(cached.image.rgba.len() / 4 + header.len() + 16);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&u32::try_from(header.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(&header);
    let mut encoder = png::Encoder::new(&mut out, cached.image.width, cached.image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Fast);
    let mut writer = encoder.write_header().map_err(io::Error::other)?;
    writer
        .write_image_data(&cached.image.rgba)
        .map_err(io::Error::other)?;
    writer.finish().map_err(io::Error::other)?;
    Ok(out)
}

fn decode(bytes: &[u8]) -> Option<Cached> {
    let rest = bytes.strip_prefix(MAGIC)?;
    let (len, rest) = rest.split_first_chunk::<4>()?;
    let len = u32::from_le_bytes(*len) as usize;
    let (header, png_bytes) = (rest.get(..len)?, rest.get(len..)?);
    let header: Header = serde_json::from_slice(header).ok()?;
    let mut reader = png::Decoder::new(Cursor::new(png_bytes)).read_info().ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut rgba).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    rgba.truncate(info.buffer_size());
    let image = Image {
        width: info.width,
        height: info.height,
        rgba,
    };
    image.is_valid().then_some(Cached {
        kind: header.kind,
        meta: header.meta,
        image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture() -> Cached {
        let rgba: Vec<u8> = (0..4 * 4 * 4).map(|i| (i * 7) as u8).collect();
        Cached {
            kind: "texture".into(),
            meta: vec![("size".into(), "4 x 4".into())],
            image: Image {
                width: 4,
                height: 4,
                rgba,
            },
        }
    }

    #[test]
    fn a_stored_picture_comes_back_identical() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ThumbCache::open_in(dir.path().join("c"), 1 << 20, None).unwrap();
        let key = Key::new("data/dx11/a.g4tx", "dx11.cpk", 1234, 0);
        assert!(cache.get(&key).is_none());
        cache.put(&key, &picture()).unwrap();
        assert_eq!(cache.get(&key), Some(picture()));
    }

    #[test]
    fn every_part_of_the_identity_changes_the_key() {
        let base = Key::new("a", "c", 1, 0);
        assert_ne!(base, Key::new("b", "c", 1, 0));
        assert_ne!(base, Key::new("a", "d", 1, 0));
        assert_ne!(base, Key::new("a", "c", 2, 0));
        assert_ne!(base, Key::new("a", "c", 1, 1));
        assert_eq!(base, Key::new("a", "c", 1, 0));
        // `ab` + `c` and `a` + `bc` must not collide.
        assert_ne!(Key::new("ab", "c", 1, 0), Key::new("a", "bc", 1, 0));
    }

    #[test]
    fn refuses_a_location_inside_the_game_directory() {
        let game = tempfile::tempdir().unwrap();
        let err = ThumbCache::open_in(game.path().join("cache"), 1 << 20, Some(game.path()))
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn a_damaged_file_is_a_miss_and_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ThumbCache::open_in(dir.path().to_owned(), 1 << 20, None).unwrap();
        let key = Key::new("a", "c", 1, 0);
        cache.put(&key, &picture()).unwrap();
        fs::write(cache.file(&key), b"NFM1garbage").unwrap();
        assert!(cache.get(&key).is_none());
        assert!(!cache.file(&key).exists());
    }

    #[test]
    fn pruning_keeps_the_newest_entries_under_budget() {
        let dir = tempfile::tempdir().unwrap();
        let one = encode(&picture()).unwrap().len() as u64;
        let cache = ThumbCache::open_in(dir.path().to_owned(), one * 3, None).unwrap();
        for i in 0..8 {
            cache
                .put(&Key::new("a", "c", i, 0), &picture())
                .unwrap();
        }
        let left = cache.prune();
        assert!(left <= one * 3, "{left} > {}", one * 3);
    }
}
