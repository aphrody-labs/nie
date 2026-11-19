//! Windows Projected File System provider: the manifest appears as ordinary NTFS files and folders
//! under the virtualization root. Placeholders are real NTFS entries that survive reboots; the first
//! read of a range asks this provider, which answers from the verified page cache or the network, and
//! ProjFS stores the bytes in the file. Unread ranges stay placeholders; files the user writes become
//! ordinary local files.
use crate::link::Link;
use crate::proto::{Node, Tree};
use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows::Win32::Foundation::{
    CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, ERROR_IO_DEVICE, ERROR_OUTOFMEMORY,
    ERROR_REPARSE_POINT_ENCOUNTERED, HANDLE, WAIT_OBJECT_0,
};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows::Win32::Storage::ProjectedFileSystem::*;
use windows::Win32::System::Console::SetConsoleCtrlHandler;
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject,
};
use windows::core::{BOOL, GUID, HRESULT, PCWSTR, w};

const FILETIME_UNIX_EPOCH: i64 = 116_444_736_000_000_000;
/// Pages are fetched and written this many at a time so a huge read does not hold the whole range.
const WRITE_GROUP_PAGES: u32 = 32;

struct Enumeration {
    entries: Vec<(String, Node)>,
    cursor: usize,
    pattern: Option<Vec<u16>>,
    started: bool,
}

struct Provider {
    link: Arc<Link>,
    tree: Tree,
    enums: Mutex<HashMap<u128, Enumeration>>,
    content_id: [u8; 128],
}

fn filetime(unix_ns: i64) -> i64 {
    unix_ns / 100 + FILETIME_UNIX_EPOCH
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn win32(code: windows::Win32::Foundation::WIN32_ERROR) -> HRESULT {
    HRESULT::from_win32(code.0)
}

/// Relative path of the callback target, `/`-separated.
unsafe fn rel_path(data: *const PRJ_CALLBACK_DATA) -> String {
    let p = unsafe { (*data).FilePathName };
    if p.is_null() {
        return String::new();
    }
    unsafe { p.to_string() }.unwrap_or_default().replace('\\', "/")
}

unsafe fn provider<'a>(data: *const PRJ_CALLBACK_DATA) -> &'a Provider {
    unsafe { &*((*data).InstanceContext as *const Provider) }
}

fn guard(f: impl FnOnce() -> HRESULT) -> HRESULT {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).unwrap_or_else(|_| win32(ERROR_IO_DEVICE))
}

fn basic_info(p: &Provider, node: Node) -> PRJ_FILE_BASIC_INFO {
    match node {
        Node::Dir => PRJ_FILE_BASIC_INFO {
            IsDirectory: true,
            FileAttributes: FILE_ATTRIBUTE_DIRECTORY.0,
            ..Default::default()
        },
        Node::File(i) => {
            let f = &p.link.manifest.files[i];
            let t = filetime(f.mtime_ns);
            PRJ_FILE_BASIC_INFO {
                IsDirectory: false,
                FileSize: i64::try_from(f.size).unwrap_or(i64::MAX),
                CreationTime: t,
                LastAccessTime: t,
                LastWriteTime: t,
                ChangeTime: t,
                FileAttributes: FILE_ATTRIBUTE_NORMAL.0,
            }
        }
    }
}

unsafe extern "system" fn start_enum(data: *const PRJ_CALLBACK_DATA, id: *const GUID) -> HRESULT {
    guard(|| {
        let (p, path) = unsafe { (provider(data), rel_path(data)) };
        let Some(entries) = p.tree.children(&path) else {
            return win32(ERROR_FILE_NOT_FOUND);
        };
        let session = Enumeration { entries: entries.to_vec(), cursor: 0, pattern: None, started: false };
        p.enums.lock().unwrap_or_else(std::sync::PoisonError::into_inner).insert(unsafe { (*id).to_u128() }, session);
        HRESULT(0)
    })
}

unsafe extern "system" fn end_enum(data: *const PRJ_CALLBACK_DATA, id: *const GUID) -> HRESULT {
    guard(|| {
        let p = unsafe { provider(data) };
        p.enums.lock().unwrap_or_else(std::sync::PoisonError::into_inner).remove(&unsafe { (*id).to_u128() });
        HRESULT(0)
    })
}

unsafe extern "system" fn get_enum(
    data: *const PRJ_CALLBACK_DATA,
    id: *const GUID,
    search: PCWSTR,
    buffer: PRJ_DIR_ENTRY_BUFFER_HANDLE,
) -> HRESULT {
    guard(|| {
        let p = unsafe { provider(data) };
        let flags = unsafe { (*data).Flags };
        let mut enums = p.enums.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(e) = enums.get_mut(&unsafe { (*id).to_u128() }) else {
            return win32(ERROR_FILE_NOT_FOUND);
        };
        if flags.0 & PRJ_CB_DATA_FLAG_ENUM_RESTART_SCAN.0 != 0 {
            e.cursor = 0;
            e.started = false;
        }
        if !e.started {
            e.started = true;
            e.pattern = (!search.is_null()).then(|| unsafe { search.as_wide() }.iter().copied().chain([0]).collect());
        }
        let single = flags.0 & PRJ_CB_DATA_FLAG_ENUM_RETURN_SINGLE_ENTRY.0 != 0;
        let mut added = 0;
        while let Some((name, node)) = e.entries.get(e.cursor) {
            let wname = wide(name);
            let matches = e.pattern.as_ref().is_none_or(|pat| unsafe {
                PrjFileNameMatch(PCWSTR(wname.as_ptr()), PCWSTR(pat.as_ptr()))
            });
            if matches {
                let info = basic_info(p, *node);
                if let Err(err) = unsafe { PrjFillDirEntryBuffer(PCWSTR(wname.as_ptr()), Some(&info), buffer) } {
                    if err.code() == win32(ERROR_INSUFFICIENT_BUFFER) && added > 0 {
                        return HRESULT(0);
                    }
                    return err.code();
                }
                added += 1;
            }
            e.cursor += 1;
            if single && added > 0 {
                break;
            }
        }
        HRESULT(0)
    })
}

unsafe extern "system" fn get_placeholder(data: *const PRJ_CALLBACK_DATA) -> HRESULT {
    guard(|| {
        let (p, path) = unsafe { (provider(data), rel_path(data)) };
        let Some((canonical, node)) = p.tree.lookup(&path) else {
            return win32(ERROR_FILE_NOT_FOUND);
        };
        let mut info = PRJ_PLACEHOLDER_INFO { FileBasicInfo: basic_info(p, node), ..Default::default() };
        info.VersionInfo.ProviderID[..8].copy_from_slice(b"nie-link");
        info.VersionInfo.ContentID = p.content_id;
        let name = wide(&canonical.replace('/', "\\"));
        match unsafe {
            PrjWritePlaceholderInfo(
                (*data).NamespaceVirtualizationContext,
                PCWSTR(name.as_ptr()),
                &info,
                std::mem::size_of::<PRJ_PLACEHOLDER_INFO>() as u32,
            )
        } {
            Ok(()) => HRESULT(0),
            Err(e) => e.code(),
        }
    })
}

unsafe extern "system" fn get_file_data(data: *const PRJ_CALLBACK_DATA, offset: u64, length: u32) -> HRESULT {
    guard(|| {
        let (p, path) = unsafe { (provider(data), rel_path(data)) };
        if std::env::var_os("NIE_LINK_DEBUG").is_some() {
            eprintln!("nie-link: data {path} offset {offset} length {length}");
        }
        let Some((_, Node::File(idx))) = p.tree.lookup(&path) else {
            return win32(ERROR_FILE_NOT_FOUND);
        };
        let m = &p.link.manifest;
        let file = &m.files[idx];
        let ps = u64::from(m.page_size);
        let end = (offset + u64::from(length)).min(file.size);
        if offset >= end {
            return HRESULT(0);
        }
        let (first, last) = ((offset / ps) as u32, ((end - 1) / ps) as u32);
        let (ctx, stream) = unsafe { ((*data).NamespaceVirtualizationContext, (*data).DataStreamId) };
        let mut group = first;
        while group <= last {
            let group_end = last.min(group + WRITE_GROUP_PAGES - 1);
            let pages = match p.link.pages(file, group, group_end, false) {
                Ok(pages) => pages,
                Err(e) => {
                    eprintln!("nie-link: read {path} pages {group}..{group_end}: {e}");
                    return win32(ERROR_IO_DEVICE);
                }
            };
            for (i, page) in pages.iter().enumerate() {
                let at = (u64::from(group) + i as u64) * ps;
                let buf = unsafe { PrjAllocateAlignedBuffer(ctx, page.len()) };
                if buf.is_null() {
                    return win32(ERROR_OUTOFMEMORY);
                }
                unsafe { std::ptr::copy_nonoverlapping(page.as_ptr(), buf.cast::<u8>(), page.len()) };
                let r = unsafe { PrjWriteFileData(ctx, &stream, buf, at, page.len() as u32) };
                unsafe { PrjFreeAlignedBuffer(buf) };
                if let Err(e) = r {
                    return e.code();
                }
            }
            group = group_end + 1;
        }
        HRESULT(0)
    })
}

static STOP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

unsafe extern "system" fn ctrl_handler(_: u32) -> BOOL {
    if let Some(h) = STOP.get() {
        let _ = unsafe { SetEvent(HANDLE(*h as *mut core::ffi::c_void)) };
    }
    BOOL(1)
}

/// Name of the event `uninstall` signals to stop a running mount.
pub const STOP_EVENT: PCWSTR = w!("Local\\nie-link-stop");

/// Mount `link` at `root` and serve until stopped (Ctrl-C, console close or the stop event).
pub fn run(link: Arc<Link>, root: &Path, record: Option<&str>, prefetch: Option<&str>) -> Result<()> {
    std::fs::create_dir_all(root).with_context(|| root.display().to_string())?;
    // One mount per user session.
    let _single = unsafe { CreateMutexW(None, true, w!("Local\\nie-link-mount")) }.context("mutex")?;
    if windows::core::Error::from_thread().code() == win32(windows::Win32::Foundation::ERROR_ALREADY_EXISTS) {
        bail!("nie-link is already mounted in this session");
    }

    let guid_file = crate::config::state_dir().join("instance.guid");
    let guid = match std::fs::read_to_string(&guid_file).ok().and_then(|s| s.trim().parse::<u128>().ok()) {
        Some(g) => GUID::from_u128(g),
        None => {
            let seed = format!("{:?}{}{}", std::time::SystemTime::now(), std::process::id(), root.display());
            let g = u128::from_le_bytes(blake3::hash(seed.as_bytes()).as_bytes()[..16].try_into().unwrap_or([7; 16]));
            std::fs::write(&guid_file, g.to_string())?;
            GUID::from_u128(g)
        }
    };
    let wroot = wide(&root.display().to_string());
    if let Err(e) = unsafe { PrjMarkDirectoryAsPlaceholder(PCWSTR(wroot.as_ptr()), PCWSTR::null(), None, &guid) }
        && e.code() != win32(ERROR_REPARSE_POINT_ENCOUNTERED)
    {
        bail!("PrjMarkDirectoryAsPlaceholder {}: {e}", root.display());
    }

    let mut content_id = [0u8; 128];
    let id = link.manifest.manifest_id.as_bytes();
    content_id[..id.len().min(128)].copy_from_slice(&id[..id.len().min(128)]);
    let tree = Tree::build(&link.manifest, |a, b| {
        let (wa, wb) = (wide(a), wide(b));
        unsafe { PrjFileNameCompare(PCWSTR(wa.as_ptr()), PCWSTR(wb.as_ptr())) }.cmp(&0)
    });
    let state = Box::new(Provider { link: link.clone(), tree, enums: Mutex::new(HashMap::new()), content_id });

    let callbacks = PRJ_CALLBACKS {
        StartDirectoryEnumerationCallback: Some(start_enum),
        EndDirectoryEnumerationCallback: Some(end_enum),
        GetDirectoryEnumerationCallback: Some(get_enum),
        GetPlaceholderInfoCallback: Some(get_placeholder),
        GetFileDataCallback: Some(get_file_data),
        ..Default::default()
    };
    let options = PRJ_STARTVIRTUALIZING_OPTIONS {
        Flags: PRJ_FLAG_USE_NEGATIVE_PATH_CACHE,
        PoolThreadCount: 16,
        ConcurrentThreadCount: 16,
        ..Default::default()
    };
    let ctx = unsafe {
        PrjStartVirtualizing(PCWSTR(wroot.as_ptr()), &callbacks, Some(&*state as *const Provider as *const core::ffi::c_void), Some(&options))
    }
    .with_context(|| format!("PrjStartVirtualizing {}", root.display()))?;

    let stop = unsafe { CreateEventW(None, true, false, STOP_EVENT) }.context("stop event")?;
    let _ = STOP.set(stop.0 as usize);
    unsafe { SetConsoleCtrlHandler(Some(ctrl_handler), true) }.ok();
    eprintln!(
        "nie-link: {} files of build {} at {}",
        link.manifest.files.len(),
        link.manifest.build_id,
        root.display()
    );

    if let Some(name) = record {
        link.record_trace();
        let (l, name) = (link.clone(), name.to_owned());
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(5));
                let _ = l.save_trace(&name);
            }
        });
    }
    if let Some(name) = prefetch {
        let (l, name) = (link.clone(), name.to_owned());
        std::thread::spawn(move || match l.load_trace(&name) {
            Ok(trace) => {
                let (fetched, failed) = l.prefetch(&trace, 8);
                eprintln!("nie-link: prefetch {name}: {fetched} pages fetched, {failed} failed");
            }
            Err(e) => eprintln!("nie-link: prefetch {name}: {e:#}"),
        });
    }
    // Keep the host measurements and cache bitmaps fresh.
    {
        let l = link.clone();
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_secs(30));
                l.router.probe(&l.manifest.manifest_id);
            }
        });
    }

    let waited = unsafe { WaitForSingleObject(stop, INFINITE) };
    debug_assert_eq!(waited, WAIT_OBJECT_0);
    unsafe { PrjStopVirtualizing(ctx) };
    if let Some(name) = record {
        let n = link.save_trace(name)?;
        eprintln!("nie-link: trace {name}: {n} pages");
    }
    let _ = unsafe { CloseHandle(stop) };
    drop(state);
    Ok(())
}
