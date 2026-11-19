//! SFTP v3 over the real filesystem, as the account the daemon runs as.

use russh_sftp::protocol::{
    Attrs, Data, File, FileAttributes, Handle, Name, OpenFlags, Status, StatusCode, Version,
};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

enum Open {
    File(fs::File),
    Dir { path: PathBuf, sent: bool },
}

pub struct Sftp {
    home: PathBuf,
    next: u64,
    handles: HashMap<String, Open>,
}

impl Sftp {
    pub fn new(home: PathBuf) -> Self {
        Self {
            home,
            next: 0,
            handles: HashMap::new(),
        }
    }

    fn path(&self, path: &str) -> PathBuf {
        let path = if path.is_empty() { "." } else { path };
        let path = Path::new(path);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.home.join(path)
        }
    }

    fn register(&mut self, id: u32, open: Open) -> Handle {
        self.next += 1;
        let handle = format!("h{}", self.next);
        self.handles.insert(handle.clone(), open);
        Handle { id, handle }
    }
}

fn status(error: &std::io::Error) -> StatusCode {
    use std::io::ErrorKind::{NotFound, PermissionDenied};
    match error.kind() {
        NotFound => StatusCode::NoSuchFile,
        PermissionDenied => StatusCode::PermissionDenied,
        _ => StatusCode::Failure,
    }
}

fn ok(id: u32) -> Status {
    Status {
        id,
        status_code: StatusCode::Ok,
        error_message: "Ok".into(),
        language_tag: "en-US".into(),
    }
}

impl russh_sftp::server::Handler for Sftp {
    type Error = StatusCode;

    fn unimplemented(&self) -> Self::Error {
        StatusCode::OpUnsupported
    }

    async fn init(
        &mut self,
        _version: u32,
        _extensions: HashMap<String, String>,
    ) -> Result<Version, Self::Error> {
        Ok(Version::new())
    }

    async fn open(
        &mut self,
        id: u32,
        filename: String,
        flags: OpenFlags,
        attrs: FileAttributes,
    ) -> Result<Handle, Self::Error> {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = fs::OpenOptions::from(flags);
        if flags.contains(OpenFlags::CREATE) {
            options.mode(attrs.permissions.map_or(0o644, |p| p & 0o7777));
        }
        let file = options.open(self.path(&filename)).map_err(|e| status(&e))?;
        Ok(self.register(id, Open::File(file)))
    }

    async fn close(&mut self, id: u32, handle: String) -> Result<Status, Self::Error> {
        self.handles.remove(&handle);
        Ok(ok(id))
    }

    async fn read(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        len: u32,
    ) -> Result<Data, Self::Error> {
        let Some(Open::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset)).map_err(|e| status(&e))?;
        let mut data = Vec::new();
        file.take(u64::from(len).min(256 * 1024))
            .read_to_end(&mut data)
            .map_err(|e| status(&e))?;
        if data.is_empty() {
            return Err(StatusCode::Eof);
        }
        Ok(Data { id, data })
    }

    async fn write(
        &mut self,
        id: u32,
        handle: String,
        offset: u64,
        data: Vec<u8>,
    ) -> Result<Status, Self::Error> {
        let Some(Open::File(file)) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        file.seek(SeekFrom::Start(offset)).map_err(|e| status(&e))?;
        file.write_all(&data).map_err(|e| status(&e))?;
        Ok(ok(id))
    }

    async fn lstat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let meta = fs::symlink_metadata(self.path(&path)).map_err(|e| status(&e))?;
        Ok(Attrs { id, attrs: FileAttributes::from(&meta) })
    }

    async fn fstat(&mut self, id: u32, handle: String) -> Result<Attrs, Self::Error> {
        let meta = match self.handles.get(&handle) {
            Some(Open::File(file)) => file.metadata(),
            Some(Open::Dir { path, .. }) => fs::metadata(path),
            None => return Err(StatusCode::Failure),
        }
        .map_err(|e| status(&e))?;
        Ok(Attrs { id, attrs: FileAttributes::from(&meta) })
    }

    async fn stat(&mut self, id: u32, path: String) -> Result<Attrs, Self::Error> {
        let meta = fs::metadata(self.path(&path)).map_err(|e| status(&e))?;
        Ok(Attrs { id, attrs: FileAttributes::from(&meta) })
    }

    async fn setstat(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        use std::os::unix::fs::PermissionsExt;
        let path = self.path(&path);
        if let Some(mode) = attrs.permissions {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o7777))
                .map_err(|e| status(&e))?;
        }
        if let Some(size) = attrs.size {
            fs::OpenOptions::new()
                .write(true)
                .open(&path)
                .and_then(|f| f.set_len(size))
                .map_err(|e| status(&e))?;
        }
        Ok(ok(id))
    }

    async fn fsetstat(
        &mut self,
        id: u32,
        handle: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        use std::os::unix::fs::PermissionsExt;
        let Some(Open::File(file)) = self.handles.get(&handle) else {
            return Err(StatusCode::Failure);
        };
        if let Some(mode) = attrs.permissions {
            file.set_permissions(fs::Permissions::from_mode(mode & 0o7777))
                .map_err(|e| status(&e))?;
        }
        if let Some(size) = attrs.size {
            file.set_len(size).map_err(|e| status(&e))?;
        }
        Ok(ok(id))
    }

    async fn opendir(&mut self, id: u32, path: String) -> Result<Handle, Self::Error> {
        let path = self.path(&path);
        if !fs::metadata(&path).map_err(|e| status(&e))?.is_dir() {
            return Err(StatusCode::NoSuchFile);
        }
        Ok(self.register(id, Open::Dir { path, sent: false }))
    }

    async fn readdir(&mut self, id: u32, handle: String) -> Result<Name, Self::Error> {
        let Some(Open::Dir { path, sent }) = self.handles.get_mut(&handle) else {
            return Err(StatusCode::Failure);
        };
        if std::mem::replace(sent, true) {
            return Err(StatusCode::Eof);
        }
        let mut files = Vec::new();
        for entry in fs::read_dir(&*path).map_err(|e| status(&e))? {
            let entry = entry.map_err(|e| status(&e))?;
            let attrs = entry
                .metadata()
                .map(|m| FileAttributes::from(&m))
                .map_err(|e| status(&e))?;
            files.push(File::new(entry.file_name().to_string_lossy().into_owned(), attrs));
        }
        Ok(Name { id, files })
    }

    async fn remove(&mut self, id: u32, filename: String) -> Result<Status, Self::Error> {
        fs::remove_file(self.path(&filename)).map_err(|e| status(&e))?;
        Ok(ok(id))
    }

    async fn mkdir(
        &mut self,
        id: u32,
        path: String,
        attrs: FileAttributes,
    ) -> Result<Status, Self::Error> {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .mode(attrs.permissions.map_or(0o755, |p| p & 0o7777))
            .create(self.path(&path))
            .map_err(|e| status(&e))?;
        Ok(ok(id))
    }

    async fn rmdir(&mut self, id: u32, path: String) -> Result<Status, Self::Error> {
        fs::remove_dir(self.path(&path)).map_err(|e| status(&e))?;
        Ok(ok(id))
    }

    async fn realpath(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let resolved = self.path(&path);
        let resolved = fs::canonicalize(&resolved).unwrap_or(resolved);
        Ok(Name {
            id,
            files: vec![File::dummy(resolved.to_string_lossy().into_owned())],
        })
    }

    async fn rename(
        &mut self,
        id: u32,
        oldpath: String,
        newpath: String,
    ) -> Result<Status, Self::Error> {
        fs::rename(self.path(&oldpath), self.path(&newpath)).map_err(|e| status(&e))?;
        Ok(ok(id))
    }

    async fn readlink(&mut self, id: u32, path: String) -> Result<Name, Self::Error> {
        let target = fs::read_link(self.path(&path)).map_err(|e| status(&e))?;
        Ok(Name {
            id,
            files: vec![File::dummy(target.to_string_lossy().into_owned())],
        })
    }

    async fn symlink(
        &mut self,
        id: u32,
        linkpath: String,
        targetpath: String,
    ) -> Result<Status, Self::Error> {
        std::os::unix::fs::symlink(&targetpath, self.path(&linkpath)).map_err(|e| status(&e))?;
        Ok(ok(id))
    }
}
