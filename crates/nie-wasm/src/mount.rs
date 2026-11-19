//! The browser's mount of the game installation.
//!
//! Two transports implement [`nie_formats::byte_source::SourceTree`]:
//!
//! - [`DirectoryTree`]: the File System Access API on the folder the user picked. The game never
//!   leaves the machine; the page reads byte ranges of the packs in place.
//! - [`HttpTree`]: HTTP `Range` requests against the user's own `nie serve` (`/v1/vfs/stream`),
//!   with a bearer token.
//!
//! Either feeds the same [`SourceVfs`], whose code (index decryption, pack tables, CRILAYLA) is
//! the one the native VFS runs. [`GameMount`] is what the page holds.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use js_sys::{ArrayBuffer, Uint8Array};
use nie_formats::FormatError;
use nie_formats::byte_source::{ByteSource, SourceTree};
use nie_formats::source_vfs::{MemoryVfs, SourceVfs};
use nie_wiki_vfs::Wiki;
use serde_json::json;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Blob, FileSystemDirectoryHandle, FileSystemFileHandle, Headers, Request, RequestInit, Response,
    WorkerGlobalScope,
};

fn js_error(value: JsValue) -> FormatError {
    let message = value
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| value.as_string())
        .unwrap_or_else(|| format!("{value:?}"));
    FormatError::Malformed(message)
}

async fn settle<T: JsCast>(promise: js_sys::Promise) -> Result<T, FormatError> {
    JsFuture::from(promise)
        .await
        .map_err(js_error)?
        .dyn_into::<T>()
        .map_err(|_| FormatError::Corrupt("unexpected JavaScript value"))
}

fn bytes_of(buffer: &ArrayBuffer) -> Vec<u8> {
    Uint8Array::new(buffer).to_vec()
}

/// A folder picked with `showDirectoryPicker()` (or any `FileSystemDirectoryHandle`, such as the
/// origin-private one a test fills).
pub struct DirectoryTree(FileSystemDirectoryHandle);

/// One file of a [`DirectoryTree`], read by `Blob.slice`.
pub struct BlobSource(Blob);

impl SourceTree for DirectoryTree {
    type File = BlobSource;

    async fn open(&self, path: &str) -> Result<BlobSource, FormatError> {
        let mut directory = self.0.clone();
        let mut parts = path.split('/').filter(|p| !p.is_empty()).peekable();
        while let Some(part) = parts.next() {
            if parts.peek().is_some() {
                directory = settle(directory.get_directory_handle(part)).await?;
            } else {
                let handle: FileSystemFileHandle = settle(directory.get_file_handle(part)).await?;
                let file: web_sys::File = settle(handle.get_file()).await?;
                return Ok(BlobSource(file.into()));
            }
        }
        Err(FormatError::Corrupt("empty path"))
    }
}

impl ByteSource for BlobSource {
    async fn len(&self) -> Result<u64, FormatError> {
        Ok(self.0.size() as u64)
    }

    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        let end = offset
            .checked_add(len as u64)
            .ok_or(FormatError::Corrupt("range overflow"))?;
        if end > self.0.size() as u64 {
            return Err(FormatError::TooShort {
                got: self.0.size() as usize,
                need: end as usize,
            });
        }
        let slice = self
            .0
            .slice_with_f64_and_f64(offset as f64, end as f64)
            .map_err(js_error)?;
        let buffer: ArrayBuffer = settle(slice.array_buffer()).await?;
        Ok(bytes_of(&buffer))
    }
}

/// The `/v1/vfs/stream` endpoint of a `nie serve`.
pub struct HttpTree {
    base: String,
    token: Option<String>,
}

/// One file behind that endpoint, read with `Range` requests.
pub struct HttpFile {
    url: String,
    token: Option<String>,
    len: u64,
}

fn encode_component(text: &str) -> String {
    String::from(js_sys::encode_uri_component(text))
}

async fn fetch(
    url: &str,
    token: Option<&str>,
    range: Option<(u64, u64)>,
) -> Result<Response, FormatError> {
    let headers = Headers::new().map_err(js_error)?;
    if let Some((first, last)) = range {
        headers
            .set("Range", &format!("bytes={first}-{last}"))
            .map_err(js_error)?;
    }
    if let Some(token) = token {
        headers
            .set("Authorization", &format!("Bearer {token}"))
            .map_err(js_error)?;
    }
    let init = RequestInit::new();
    init.set_method("GET");
    init.set_headers(&headers);
    let request = Request::new_with_str_and_init(url, &init).map_err(js_error)?;
    // `fetch` is a method of `window` and of every worker scope; both answer to this binding.
    let scope: WorkerGlobalScope = js_sys::global().unchecked_into();
    settle(scope.fetch_with_request(&request)).await
}

impl SourceTree for HttpTree {
    type File = HttpFile;

    async fn open(&self, path: &str) -> Result<HttpFile, FormatError> {
        let url = format!(
            "{}/v1/vfs/stream?path={}",
            self.base.trim_end_matches('/'),
            encode_component(path)
        );
        // One byte tells whether the file exists and, in `Content-Range`, how long it is.
        let response = fetch(&url, self.token.as_deref(), Some((0, 0))).await?;
        let status = response.status();
        if status == 416 {
            return Ok(HttpFile {
                url,
                token: self.token.clone(),
                len: 0,
            });
        }
        if status != 206 {
            return Err(FormatError::Malformed(format!(
                "{path}: the server answered {status}"
            )));
        }
        let total = response
            .headers()
            .get("Content-Range")
            .map_err(js_error)?
            .and_then(|v| v.rsplit('/').next().and_then(|n| n.parse::<u64>().ok()))
            .ok_or(FormatError::Corrupt("the server sent no Content-Range total"))?;
        Ok(HttpFile {
            url,
            token: self.token.clone(),
            len: total,
        })
    }
}

impl ByteSource for HttpFile {
    async fn len(&self) -> Result<u64, FormatError> {
        Ok(self.len)
    }

    async fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>, FormatError> {
        if len == 0 {
            return Ok(Vec::new());
        }
        let last = offset
            .checked_add(len as u64 - 1)
            .ok_or(FormatError::Corrupt("range overflow"))?;
        if last >= self.len {
            return Err(FormatError::TooShort {
                got: self.len as usize,
                need: last as usize + 1,
            });
        }
        let response = fetch(&self.url, self.token.as_deref(), Some((offset, last))).await?;
        if response.status() != 206 {
            return Err(FormatError::Malformed(format!(
                "range read answered {}",
                response.status()
            )));
        }
        let buffer: ArrayBuffer = settle(response.array_buffer().map_err(js_error)?).await?;
        let bytes = bytes_of(&buffer);
        if bytes.len() != len {
            return Err(FormatError::TooShort {
                got: bytes.len(),
                need: len,
            });
        }
        Ok(bytes)
    }
}

enum Mounted {
    Directory(SourceVfs<DirectoryTree>),
    Http(SourceVfs<HttpTree>),
}

macro_rules! on {
    ($self:expr, $v:ident => $body:expr) => {
        match &$self.vfs {
            Mounted::Directory($v) => $body,
            Mounted::Http($v) => $body,
        }
    };
}

/// The mounted installation, held by the page.
#[wasm_bindgen]
pub struct GameMount {
    vfs: Mounted,
    kind: &'static str,
    memory: Arc<MemoryVfs>,
    wiki: RefCell<Option<Arc<Wiki>>>,
}

fn fail(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

#[wasm_bindgen]
impl GameMount {
    /// Mounts the installation held by a directory handle: the game folder or its `data` folder.
    pub async fn mount_directory(handle: FileSystemDirectoryHandle) -> Result<GameMount, JsError> {
        let vfs = SourceVfs::mount(DirectoryTree(handle)).await.map_err(fail)?;
        let memory = vfs.memory();
        Ok(Self {
            vfs: Mounted::Directory(vfs),
            kind: "directory",
            memory,
            wiki: RefCell::new(None),
        })
    }

    /// Mounts the installation served by `nie serve` at `base` (empty for the page's own origin).
    pub async fn mount_http(base: String, token: Option<String>) -> Result<GameMount, JsError> {
        let vfs = SourceVfs::mount(HttpTree { base, token })
            .await
            .map_err(fail)?;
        let memory = vfs.memory();
        Ok(Self {
            vfs: Mounted::Http(vfs),
            kind: "http",
            memory,
            wiki: RefCell::new(None),
        })
    }

    /// `{kind, entries, packs}` as JSON.
    pub fn summary(&self) -> String {
        let (entries, packs) = on!(self, v => (v.len(), v.pack_count()));
        json!({ "kind": self.kind, "entries": entries, "packs": packs }).to_string()
    }

    /// One folder of the VFS: `{dir, dirs:[{name, files}], files:[{name, path, size}]}`.
    pub fn list(&self, dir: &str) -> String {
        let prefix = if dir.is_empty() {
            String::new()
        } else {
            format!("{}/", dir.trim_matches('/'))
        };
        let mut dirs: BTreeMap<&str, usize> = BTreeMap::new();
        let mut files: Vec<(&str, u64)> = Vec::new();
        on!(self, v => {
            for (path, size) in v.entries() {
                let Some(rest) = path.strip_prefix(prefix.as_str()) else { continue };
                match rest.split_once('/') {
                    Some((folder, _)) => *dirs.entry(folder).or_default() += 1,
                    None => files.push((path, size)),
                }
            }
        });
        files.sort_unstable();
        json!({
            "dir": prefix.trim_end_matches('/'),
            "dirs": dirs.iter().map(|(name, n)| json!({ "name": name, "files": n })).collect::<Vec<_>>(),
            "files": files.iter().map(|(path, size)| json!({
                "name": path.rsplit('/').next().unwrap_or(path),
                "path": path,
                "size": size,
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// The first `limit` paths containing `query`, case-insensitively, as `{count, items}`.
    pub fn find(&self, query: &str, limit: usize) -> String {
        let needle = query.to_lowercase();
        let mut items: Vec<(&str, u64)> = Vec::new();
        on!(self, v => {
            for (path, size) in v.entries() {
                if path.to_lowercase().contains(&needle) {
                    items.push((path, size));
                }
            }
        });
        items.sort_unstable();
        let count = items.len();
        json!({
            "count": count,
            "items": items.iter().take(limit).map(|(p, s)| json!({ "path": p, "size": s })).collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// The decoded bytes of one entry.
    pub async fn read(&self, path: String) -> Result<Vec<u8>, JsError> {
        match &self.vfs {
            Mounted::Directory(v) => v.read(&path).await,
            Mounted::Http(v) => v.read(&path).await,
        }
        .map_err(fail)
    }

    /// BLAKE3 of one entry, lowercase hex: the value the native `nie vfs cat | b3sum` gives.
    pub async fn digest(&self, path: String) -> Result<String, JsError> {
        let bytes = self.read(path).await?;
        Ok(blake3::hash(&bytes).to_hex().to_string())
    }

    /// `{format, size, summary}` of one entry, from the same dispatcher as `nie vfs cat`.
    pub async fn describe(&self, path: String) -> Result<String, JsError> {
        let bytes = self.read(path.clone()).await?;
        Ok(json!({
            "format": crate::detect_format(&bytes),
            "size": bytes.len(),
            "summary": crate::vfs_content_summary(&path, &bytes),
        })
        .to_string())
    }

    /// `{status, html}` of the wiki page at `target` (`/wiki/en/characters?q=a`), computed in the
    /// page from the tables it fetches on demand.
    pub async fn wiki_page(&self, target: String) -> Result<String, JsError> {
        let calls = RefCell::new(0usize);
        let rendered = on!(self, v => v.run(&self.memory, |memory| {
            let first = {
                let mut calls = calls.borrow_mut();
                *calls += 1;
                *calls == 1
            };
            let cached = if first { self.wiki.borrow().clone() } else { None };
            let wiki = match cached {
                Some(wiki) => Ok(wiki),
                None => {
                    let shared: Arc<dyn nie_formats::vfs_read::VfsRead + Send + Sync> =
                        self.memory.clone();
                    let _ = memory;
                    Wiki::over(shared).map(Arc::new)
                }
            };
            wiki.map(|wiki| {
                let page = nie_wiki_vfs::pages::render(&wiki, &target);
                *self.wiki.borrow_mut() = Some(wiki);
                page
            })
        }).await)
        .map_err(fail)?;
        Ok(json!({ "status": rendered.status, "html": rendered.html }).to_string())
    }
}
