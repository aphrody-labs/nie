//! `ssh://` and `sftp://` game sources over pure-Rust SSH (`nie-ssh`) and the SFTP subsystem.
//!
//! Host keys are checked against `known_hosts`, identities come from `ssh-agent` and the
//! `~/.ssh` keys, and `~/.ssh/config` (`HostName`, `User`, `Port`, `IdentityFile`, `ProxyJump`,
//! `ConnectTimeout`) is honoured, all by `nie-ssh`. Credentials never travel in the URI.

use super::Result;
use nie_ssh::{Connection, HostKeyPolicy, Target};
use russh_sftp::client::{Config as SftpConfig, RawSftpSession, error::Error as SftpError};
use russh_sftp::protocol::{FileAttributes, OpenFlags, StatusCode};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task::JoinHandle;

const REQUEST_TIMEOUT_SECS: u64 = 30;
/// Read size every SFTP server accepts; reads are pipelined `WINDOW` deep.
const CHUNK: usize = 32 * 1024;
const WINDOW: usize = 64;

/// Where a source lives and how to reach it, before `~/.ssh/config` is applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SshSpec {
    pub user: Option<String>,
    pub host: String,
    pub port: Option<u16>,
    /// Remote directory holding the game; `/` is the filesystem root, a relative root is
    /// relative to the account's home directory (rclone `sftp` remotes).
    pub root: String,
    /// Apply `~/.ssh/config` to this host (`ssh://` URIs; rclone remotes carry their own settings).
    pub use_config: bool,
    pub identity_files: Vec<PathBuf>,
    pub known_hosts: Option<PathBuf>,
}

/// Splits `[user@]host[:port]`, with `[v6]` brackets.
fn split_authority(authority: &str) -> Result<(Option<String>, String, Option<u16>)> {
    nie_ssh::split_authority(authority).map_err(|e| e.to_string())
}

pub(super) fn parse_uri(source: &str) -> Result<SshSpec> {
    let rest = source
        .strip_prefix("ssh://")
        .or_else(|| source.strip_prefix("sftp://"))
        .ok_or_else(|| "not an ssh URI".to_owned())?;
    let (authority, root) = rest
        .split_once('/')
        .map_or((rest, "/".to_owned()), |(a, p)| (a, format!("/{p}")));
    let (user, host, port) = split_authority(authority)?;
    let root = root.trim_end_matches('/');
    Ok(SshSpec {
        user,
        host,
        port,
        root: if root.is_empty() { "/" } else { root }.to_owned(),
        use_config: true,
        identity_files: Vec::new(),
        known_hosts: None,
    })
}

/// `root/path`, honouring `/` and relative (home-anchored) roots.
fn join(root: &str, path: &str) -> String {
    match (root, path) {
        ("", path) => path.to_owned(),
        (root, "") => root.to_owned(),
        (root, path) if root.ends_with('/') => format!("{root}{path}"),
        (root, path) => format!("{root}/{path}"),
    }
}

pub(super) fn home_dir() -> Option<PathBuf> {
    nie_ssh::home_dir()
}

impl SshSpec {
    fn target(&self) -> Target {
        Target {
            user: self.user.clone(),
            host: self.host.clone(),
            port: self.port,
            use_config: self.use_config,
            identity_files: self.identity_files.clone(),
            known_hosts: self.known_hosts.clone(),
            jumps: Vec::new(),
        }
    }
}

struct Conn {
    sftp: Arc<RawSftpSession>,
    /// The session (jump hosts included); kept so the tunnels stay open.
    ssh: Connection,
}

impl Conn {
    fn alive(&self) -> bool {
        !self.ssh.is_closed()
    }
}

/// A lazily reconnecting SFTP view of one remote directory.
pub(super) struct Ssh {
    target: Target,
    home: PathBuf,
    root: String,
    conn: Option<Conn>,
}

fn status(error: &SftpError) -> Option<StatusCode> {
    match error {
        SftpError::Status(s) => Some(s.status_code),
        _ => None,
    }
}

/// Errors of the transport, not of the request: the session must be rebuilt.
fn lost(error: &SftpError) -> bool {
    !matches!(error, SftpError::Status(_) | SftpError::Limited(_))
}

fn describe(path: &str, error: &SftpError) -> String {
    format!("sftp {path}: {error}")
}

impl Ssh {
    /// Connects and checks that the root is a directory, so a bad source fails at mount time.
    pub async fn connect(spec: &SshSpec, home: &Path) -> Result<Self> {
        let mut ssh = Self {
            target: spec.target(),
            home: home.to_path_buf(),
            root: spec.root.clone(),
            conn: None,
        };
        let conn = ssh.dial().await?;
        let root = if ssh.root.is_empty() { "." } else { &ssh.root };
        let attrs = conn
            .sftp
            .stat(root)
            .await
            .map_err(|e| describe(root, &e))?
            .attrs;
        if !attrs.file_type().is_dir() {
            return Err(format!("sftp {root}: not a directory"));
        }
        ssh.conn = Some(conn);
        Ok(ssh)
    }

    async fn dial(&self) -> Result<Conn> {
        let ssh = Connection::connect(&self.target, &self.home, HostKeyPolicy::Strict)
            .await
            .map_err(|e| e.to_string())?;
        let stream = ssh
            .open_subsystem("sftp")
            .await
            .map_err(|e| e.to_string())?;
        let sftp = RawSftpSession::new_with_config(
            stream,
            SftpConfig {
                request_timeout_secs: REQUEST_TIMEOUT_SECS,
                ..SftpConfig::default()
            },
        );
        sftp.init().await.map_err(|e| format!("sftp init: {e}"))?;
        Ok(Conn {
            sftp: Arc::new(sftp),
            ssh,
        })
    }

    async fn session(&mut self) -> Result<Arc<RawSftpSession>> {
        if !self.conn.as_ref().is_some_and(Conn::alive) {
            self.conn = Some(self.dial().await?);
        }
        let conn = self.conn.as_ref().ok_or("ssh not connected")?;
        Ok(conn.sftp.clone())
    }

    /// Runs `op` on the live session, rebuilding it once when the transport died.
    async fn with_session<T, F, Fut>(&mut self, op: F) -> Result<T>
    where
        F: Fn(Arc<RawSftpSession>) -> Fut,
        Fut: Future<Output = std::result::Result<T, (String, bool)>>,
    {
        let mut attempt = 0;
        loop {
            let sftp = self.session().await?;
            match op(sftp).await {
                Ok(value) => return Ok(value),
                Err((message, transport)) => {
                    if transport && attempt == 0 {
                        self.conn = None;
                        attempt += 1;
                        continue;
                    }
                    if transport {
                        self.conn = None;
                    }
                    return Err(message);
                }
            }
        }
    }

    pub async fn read(&mut self, path: &str, range: Option<(u64, usize)>) -> Result<Vec<u8>> {
        let remote = join(&self.root, path);
        self.with_session(|sftp| {
            let remote = remote.clone();
            async move {
                read_file(&sftp, &remote, range)
                    .await
                    .map_err(|e| (describe(&remote, &e), lost(&e)))
            }
        })
        .await
    }

    pub async fn list(&mut self, path: &str) -> Result<Vec<String>> {
        let remote = join(&self.root, path.trim_matches('/'));
        let remote = if remote.is_empty() {
            ".".to_owned()
        } else {
            remote
        };
        self.with_session(|sftp| {
            let remote = remote.clone();
            async move {
                list_dir(&sftp, &remote)
                    .await
                    .map_err(|e| (describe(&remote, &e), lost(&e)))
            }
        })
        .await
    }
}

async fn list_dir(sftp: &RawSftpSession, dir: &str) -> std::result::Result<Vec<String>, SftpError> {
    let handle = sftp.opendir(dir).await?.handle;
    let mut names = Vec::new();
    let outcome = loop {
        match sftp.readdir(handle.as_str()).await {
            Ok(batch) => {
                for file in batch.files {
                    if file.filename == "." || file.filename == ".." {
                        continue;
                    }
                    let kind = file.attrs.file_type();
                    let is_dir = if kind.is_symlink() {
                        sftp.stat(join(dir, &file.filename))
                            .await
                            .is_ok_and(|a| a.attrs.file_type().is_dir())
                    } else {
                        kind.is_dir()
                    };
                    names.push(if is_dir {
                        format!("{}/", file.filename)
                    } else {
                        file.filename
                    });
                }
            }
            Err(e) if status(&e) == Some(StatusCode::Eof) => break Ok(()),
            Err(e) => break Err(e),
        }
    };
    let _ = sftp.close(handle).await;
    outcome?;
    names.sort();
    names.dedup();
    Ok(names)
}

async fn read_file(
    sftp: &Arc<RawSftpSession>,
    path: &str,
    range: Option<(u64, usize)>,
) -> std::result::Result<Vec<u8>, SftpError> {
    let handle = sftp
        .open(path, OpenFlags::READ, FileAttributes::empty())
        .await?
        .handle;
    let outcome = read_handle(sftp, &handle, range).await;
    let _ = sftp.close(handle).await;
    outcome
}

fn truncated(offset: u64) -> SftpError {
    SftpError::UnexpectedBehavior(format!("range read truncated at offset {offset}"))
}

/// One in-flight `chunk` of a ranged read: its offset inside the output, size and request.
struct PendingRead {
    at: usize,
    len: usize,
    task: JoinHandle<std::result::Result<Vec<u8>, SftpError>>,
}

async fn read_handle(
    sftp: &Arc<RawSftpSession>,
    handle: &str,
    range: Option<(u64, usize)>,
) -> std::result::Result<Vec<u8>, SftpError> {
    let (start, total) = match range {
        Some(range) => range,
        None => {
            let size = sftp.fstat(handle).await?.attrs.size.unwrap_or(0);
            let total = usize::try_from(size)
                .ok()
                .filter(|n| *n <= super::MAX_READ)
                .ok_or_else(|| SftpError::Limited("file exceeds 256 MiB".to_owned()))?;
            (0, total)
        }
    };
    let mut out = vec![0u8; total];
    let mut next = 0usize;
    let mut inflight: VecDeque<PendingRead> = VecDeque::with_capacity(WINDOW);
    let outcome = async {
        loop {
            while inflight.len() < WINDOW && next < total {
                let len = CHUNK.min(total - next);
                let (sftp, handle) = (sftp.clone(), handle.to_owned());
                let offset = start + next as u64;
                inflight.push_back(PendingRead {
                    at: next,
                    len,
                    task: tokio::spawn(async move {
                        sftp.read(handle, offset, len as u32).await.map(|d| d.data)
                    }),
                });
                next += len;
            }
            let Some(PendingRead { at, len, task }) = inflight.pop_front() else {
                return Ok(());
            };
            let mut data = task
                .await
                .map_err(|e| SftpError::UnexpectedBehavior(e.to_string()))??;
            data.truncate(len);
            let mut got = data.len();
            out[at..at + got].copy_from_slice(&data);
            // A server may answer short: fetch the rest of this chunk before going on.
            while got < len {
                let offset = start + (at + got) as u64;
                let more = match sftp.read(handle, offset, (len - got) as u32).await {
                    Ok(more) => more.data,
                    Err(e) if status(&e) == Some(StatusCode::Eof) => return Err(truncated(offset)),
                    Err(e) => return Err(e),
                };
                if more.is_empty() {
                    return Err(truncated(offset));
                }
                let take = more.len().min(len - got);
                out[at + got..at + got + take].copy_from_slice(&more[..take]);
                got += take;
            }
        }
    }
    .await;
    for pending in inflight {
        pending.task.abort();
    }
    match outcome {
        Err(e) if status(&e) == Some(StatusCode::Eof) => Err(truncated(start + next as u64)),
        other => other.map(|()| out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_splits_user_host_port_and_root() {
        let spec = parse_uri("ssh://prd/srv/game/data").unwrap();
        assert_eq!(
            (spec.user, spec.host, spec.port, spec.root.as_str()),
            (None, "prd".to_owned(), None, "/srv/game/data")
        );
        let spec = parse_uri("sftp://ci@host:2222/data/").unwrap();
        assert_eq!(
            (spec.user, spec.host, spec.port, spec.root.as_str()),
            (
                Some("ci".to_owned()),
                "host".to_owned(),
                Some(2222),
                "/data"
            )
        );
        let spec = parse_uri("ssh://[::1]:2200").unwrap();
        assert_eq!(
            (spec.host.as_str(), spec.port, spec.root.as_str()),
            ("::1", Some(2200), "/")
        );
        assert!(parse_uri("ssh://-oProxyCommand=x/").is_err());
        assert!(parse_uri("ssh://host:notaport/").is_err());
        assert!(parse_uri("ssh://@host/").is_err());
    }

    #[test]
    fn join_handles_absolute_and_home_relative_roots() {
        assert_eq!(join("/srv/game", "a/b"), "/srv/game/a/b");
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("game", "a"), "game/a");
        assert_eq!(join("", "a"), "a");
        assert_eq!(join("/srv", ""), "/srv");
    }

    /// A real `russh` server on 127.0.0.1 serving a temp directory; its tests use Unix paths.
    #[cfg(unix)]
    mod live {
        use super::super::*;
        use std::time::Duration;
        use russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
        use russh::keys::ssh_key::{LineEnding, PrivateKey};
        use russh::server::{Auth, Msg, Server as _, Session};
        use russh::{Channel, ChannelId};
        use russh_sftp::protocol::{
            Attrs, Data, File, Handle as SftpHandle, Name, Status, Version,
        };
        use sha2::{Digest, Sha256};
        use std::collections::{HashMap, HashSet};
        use std::net::SocketAddr;

        fn test_key(label: &str) -> PrivateKey {
            let seed: [u8; 32] = Sha256::digest(label.as_bytes()).into();
            PrivateKey::new(
                KeypairData::Ed25519(Ed25519Keypair::from_seed(&seed)),
                label,
            )
            .unwrap()
        }

        #[derive(Clone)]
        struct TestServer {
            root: PathBuf,
            client_key: russh::keys::PublicKey,
        }

        impl russh::server::Server for TestServer {
            type Handler = TestSession;

            fn new_client(&mut self, _: Option<SocketAddr>) -> TestSession {
                TestSession {
                    root: self.root.clone(),
                    client_key: self.client_key.clone(),
                    channels: HashMap::new(),
                }
            }
        }

        struct TestSession {
            root: PathBuf,
            client_key: russh::keys::PublicKey,
            channels: HashMap<ChannelId, Channel<Msg>>,
        }

        impl russh::server::Handler for TestSession {
            type Error = russh::Error;

            async fn auth_publickey(
                &mut self,
                _user: &str,
                key: russh::server::VerifiedPublicKey<'_>,
            ) -> std::result::Result<Auth, Self::Error> {
                Ok(if key.public_key.key_data() == self.client_key.key_data() {
                    Auth::Accept
                } else {
                    Auth::reject()
                })
            }

            async fn channel_open_session(
                &mut self,
                channel: Channel<Msg>,
                reply: russh::server::ChannelOpenHandle,
                _session: &mut Session,
            ) -> std::result::Result<(), Self::Error> {
                self.channels.insert(channel.id(), channel);
                reply.accept().await;
                Ok(())
            }

            async fn subsystem_request(
                &mut self,
                channel_id: ChannelId,
                name: &str,
                session: &mut Session,
            ) -> std::result::Result<(), Self::Error> {
                if name == "sftp"
                    && let Some(channel) = self.channels.remove(&channel_id)
                {
                    session.channel_success(channel_id)?;
                    let handler = TestSftp {
                        root: self.root.clone(),
                        listed: HashSet::new(),
                    };
                    russh_sftp::server::run(channel.into_stream(), handler).await;
                } else {
                    session.channel_failure(channel_id)?;
                }
                Ok(())
            }
        }

        struct TestSftp {
            root: PathBuf,
            listed: HashSet<String>,
        }

        impl TestSftp {
            fn allowed(&self, path: &str) -> std::result::Result<PathBuf, StatusCode> {
                let path = PathBuf::from(path);
                if path.starts_with(&self.root) && !path.components().any(|c| c.as_os_str() == "..")
                {
                    Ok(path)
                } else {
                    Err(StatusCode::PermissionDenied)
                }
            }

            fn attrs(path: &Path) -> std::result::Result<FileAttributes, StatusCode> {
                let meta = std::fs::metadata(path).map_err(|_| StatusCode::NoSuchFile)?;
                let mut attrs = FileAttributes::empty();
                attrs.size = Some(meta.len());
                attrs.permissions = Some(if meta.is_dir() { 0o040755 } else { 0o100644 });
                Ok(attrs)
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

        impl russh_sftp::server::Handler for TestSftp {
            type Error = StatusCode;

            fn unimplemented(&self) -> Self::Error {
                StatusCode::OpUnsupported
            }

            async fn init(
                &mut self,
                _version: u32,
                _extensions: HashMap<String, String>,
            ) -> std::result::Result<Version, Self::Error> {
                Ok(Version::new())
            }

            async fn open(
                &mut self,
                id: u32,
                filename: String,
                _flags: OpenFlags,
                _attrs: FileAttributes,
            ) -> std::result::Result<SftpHandle, Self::Error> {
                let path = self.allowed(&filename)?;
                if !path.is_file() {
                    return Err(StatusCode::NoSuchFile);
                }
                Ok(SftpHandle {
                    id,
                    handle: filename,
                })
            }

            async fn close(
                &mut self,
                id: u32,
                handle: String,
            ) -> std::result::Result<Status, Self::Error> {
                self.listed.remove(&handle);
                Ok(ok(id))
            }

            async fn read(
                &mut self,
                id: u32,
                handle: String,
                offset: u64,
                len: u32,
            ) -> std::result::Result<Data, Self::Error> {
                use std::io::{Read, Seek, SeekFrom};
                let mut file = std::fs::File::open(self.allowed(&handle)?)
                    .map_err(|_| StatusCode::NoSuchFile)?;
                file.seek(SeekFrom::Start(offset))
                    .map_err(|_| StatusCode::Failure)?;
                // Answer at most 20000 bytes per request: clients must cope with short reads.
                let mut data = Vec::new();
                file.take(u64::from(len).min(20_000))
                    .read_to_end(&mut data)
                    .map_err(|_| StatusCode::Failure)?;
                if data.is_empty() {
                    return Err(StatusCode::Eof);
                }
                Ok(Data { id, data })
            }

            async fn opendir(
                &mut self,
                id: u32,
                path: String,
            ) -> std::result::Result<SftpHandle, Self::Error> {
                if !self.allowed(&path)?.is_dir() {
                    return Err(StatusCode::NoSuchFile);
                }
                Ok(SftpHandle { id, handle: path })
            }

            async fn readdir(
                &mut self,
                id: u32,
                handle: String,
            ) -> std::result::Result<Name, Self::Error> {
                if !self.listed.insert(handle.clone()) {
                    return Err(StatusCode::Eof);
                }
                let mut files = Vec::new();
                for entry in
                    std::fs::read_dir(self.allowed(&handle)?).map_err(|_| StatusCode::Failure)?
                {
                    let entry = entry.map_err(|_| StatusCode::Failure)?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    files.push(File::new(name, Self::attrs(&entry.path())?));
                }
                Ok(Name { id, files })
            }

            async fn fstat(
                &mut self,
                id: u32,
                handle: String,
            ) -> std::result::Result<Attrs, Self::Error> {
                Ok(Attrs {
                    id,
                    attrs: Self::attrs(&self.allowed(&handle)?)?,
                })
            }

            async fn stat(
                &mut self,
                id: u32,
                path: String,
            ) -> std::result::Result<Attrs, Self::Error> {
                Ok(Attrs {
                    id,
                    attrs: Self::attrs(&self.allowed(&path)?)?,
                })
            }

            async fn realpath(
                &mut self,
                id: u32,
                path: String,
            ) -> std::result::Result<Name, Self::Error> {
                Ok(Name {
                    id,
                    files: vec![File::dummy(path)],
                })
            }
        }

        struct Fixture {
            port: u16,
            home: tempfile::TempDir,
            root: tempfile::TempDir,
            host_key: PrivateKey,
            _server: std::thread::JoinHandle<()>,
        }

        fn payload() -> Vec<u8> {
            (0..200_000u32).map(|i| (i % 251) as u8).collect()
        }

        fn fixture(authorised: bool) -> Fixture {
            let root = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(root.path().join("packs/sub")).unwrap();
            std::fs::write(root.path().join("packs/a.bin"), payload()).unwrap();
            std::fs::write(root.path().join("packs/empty.bin"), b"").unwrap();
            let home = tempfile::tempdir().unwrap();
            let ssh = home.path().join(".ssh");
            std::fs::create_dir_all(&ssh).unwrap();
            let client_key = test_key("nie-test-client");
            let key_pem = client_key.to_openssh(LineEnding::LF).unwrap();
            std::fs::write(ssh.join("id_ed25519"), key_pem.as_bytes()).unwrap();
            let host_key = test_key("nie-test-host");
            let accepted = if authorised {
                client_key.public_key().clone()
            } else {
                test_key("nie-test-other").public_key().clone()
            };
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let port = listener.local_addr().unwrap().port();
            let mut server = TestServer {
                root: root.path().to_path_buf(),
                client_key: accepted,
            };
            let config = Arc::new(russh::server::Config {
                keys: vec![host_key.clone()],
                auth_rejection_time: Duration::from_millis(1),
                auth_rejection_time_initial: Some(Duration::ZERO),
                ..Default::default()
            });
            let thread = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                    let _ = server.run_on_socket(config, &listener).await;
                });
            });
            Fixture {
                port,
                home,
                root,
                host_key,
                _server: thread,
            }
        }

        fn trust_host(f: &Fixture, key: &PrivateKey) {
            let line = format!(
                "[127.0.0.1]:{} {}\n",
                f.port,
                key.public_key().to_openssh().unwrap()
            );
            std::fs::write(f.home.path().join(".ssh/known_hosts"), line).unwrap();
        }

        fn uri(f: &Fixture) -> String {
            format!(
                "ssh://tester@127.0.0.1:{}{}",
                f.port,
                f.root.path().display()
            )
        }

        fn run<T>(f: impl Future<Output = T>) -> T {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(f)
        }

        #[test]
        fn sftp_ranges_lists_and_missing_files_over_a_real_ssh_session() {
            let f = fixture(true);
            trust_host(&f, &f.host_key);
            let spec = parse_uri(&uri(&f)).unwrap();
            run(async {
                let mut ssh = Ssh::connect(&spec, f.home.path()).await.unwrap();
                let data = payload();
                // Spans several 32 KiB requests and a server that answers 20000 bytes at a time.
                assert_eq!(
                    ssh.read("packs/a.bin", Some((1_000, 150_000)))
                        .await
                        .unwrap(),
                    data[1_000..151_000]
                );
                assert_eq!(
                    ssh.read("packs/a.bin", Some((3, 4))).await.unwrap(),
                    data[3..7]
                );
                assert_eq!(ssh.read("packs/a.bin", None).await.unwrap(), data);
                assert_eq!(ssh.read("packs/empty.bin", None).await.unwrap(), b"");
                assert!(
                    ssh.read("packs/a.bin", Some((199_990, 100)))
                        .await
                        .unwrap_err()
                        .contains("truncated")
                );
                assert!(ssh.read("packs/none.bin", None).await.is_err());
                assert_eq!(
                    ssh.list("packs").await.unwrap(),
                    ["a.bin", "empty.bin", "sub/"]
                );
                assert_eq!(ssh.list("").await.unwrap(), ["packs/"]);
            });
        }

        #[test]
        fn mount_inside_a_running_tokio_runtime_does_not_nest_runtimes() {
            let f = fixture(true);
            trust_host(&f, &f.host_key);
            let source = uri(&f);
            let home = f.home.path().to_path_buf();
            let outer = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            outer.block_on(async {
                let storage =
                    crate::vfs_remote::RemoteStorage::connect_in(&source, Some(home)).unwrap();
                assert_eq!(storage.range("packs/a.bin", 3, 4).unwrap(), payload()[3..7]);
                assert_eq!(
                    storage.list("packs").unwrap(),
                    ["a.bin", "empty.bin", "sub/"]
                );
                assert_eq!(storage.stats().1, 4);
            });
        }

        #[test]
        fn dropped_session_is_rebuilt_on_the_next_read() {
            let f = fixture(true);
            trust_host(&f, &f.host_key);
            let spec = parse_uri(&uri(&f)).unwrap();
            run(async {
                let mut ssh = Ssh::connect(&spec, f.home.path()).await.unwrap();
                ssh.conn = None;
                assert_eq!(
                    ssh.read("packs/a.bin", Some((0, 4))).await.unwrap(),
                    payload()[..4]
                );
            });
        }

        #[test]
        fn unknown_changed_and_unauthorised_hosts_are_refused() {
            let f = fixture(true);
            let spec = parse_uri(&uri(&f)).unwrap();
            let unknown = run(Ssh::connect(&spec, f.home.path())).err().unwrap();
            assert!(unknown.contains("not in known_hosts"), "{unknown}");
            trust_host(&f, &test_key("nie-test-impostor"));
            let changed = run(Ssh::connect(&spec, f.home.path())).err().unwrap();
            assert!(changed.contains("changed"), "{changed}");
            let denied = fixture(false);
            trust_host(&denied, &denied.host_key);
            let spec = parse_uri(&uri(&denied)).unwrap();
            let refused = run(Ssh::connect(&spec, denied.home.path())).err().unwrap();
            assert!(refused.contains("authentication"), "{refused}");
        }
    }
}
