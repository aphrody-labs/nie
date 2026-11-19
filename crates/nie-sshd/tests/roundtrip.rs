//! nie-ssh against nie-sshd on loopback: authentication, exec, SFTP, forwarding, host keys.
#![cfg(unix)]

use nie_ssh::russh::keys::ssh_key::private::{Ed25519Keypair, KeypairData};
use nie_ssh::russh::keys::ssh_key::{LineEnding, PrivateKey};
use nie_ssh::{Connection, Forward, HostKeyPolicy, Target};
use nie_sshd::{AuthorizedKeys, Listener, Settings, serve};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn key(seed: u8) -> PrivateKey {
    PrivateKey::new(
        KeypairData::Ed25519(Ed25519Keypair::from_seed(&[seed; 32])),
        format!("test-{seed}"),
    )
    .unwrap()
}

struct Fixture {
    port: u16,
    /// The client's `$HOME`: ~/.ssh holds the identity and known_hosts.
    home: tempfile::TempDir,
    /// The server account's home: SFTP paths resolve here.
    server_home: tempfile::TempDir,
    host_pub: String,
}

async fn start(authorised_seed: u8, client_seed: u8) -> Fixture {
    let home = tempfile::tempdir().unwrap();
    let ssh = home.path().join(".ssh");
    std::fs::create_dir_all(&ssh).unwrap();
    let identity = key(client_seed);
    std::fs::write(
        ssh.join("id_ed25519"),
        identity.to_openssh(LineEnding::LF).unwrap().as_bytes(),
    )
    .unwrap();
    let server_home = tempfile::tempdir().unwrap();
    let state = tempfile::tempdir().unwrap();
    let host_key = nie_sshd::host_key_or_create(&state.path().join("host_ed25519")).unwrap();
    let host_pub = host_key.public_key().to_openssh().unwrap();
    let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address: SocketAddr = probe.local_addr().unwrap();
    drop(probe);
    let authorized = AuthorizedKeys::parse(&key(authorised_seed).public_key().to_openssh().unwrap());
    let home_path = server_home.path().to_path_buf();
    tokio::spawn(async move {
        let _state = state;
        let _ = serve(Settings {
            host_key,
            authorized,
            user: "tester".into(),
            home: home_path,
            shell: "/bin/sh".into(),
            allow_forwarding: true,
            listener: Listener::Addresses(vec![address]),
        })
        .await;
    });
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(address).await.is_ok() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    Fixture {
        port: address.port(),
        home,
        server_home,
        host_pub,
    }
}

fn target(fixture: &Fixture, known: bool) -> Target {
    let mut target = Target::parse(&format!("tester@127.0.0.1:{}", fixture.port)).unwrap();
    target.use_config = false;
    target.identity_files = vec![fixture.home.path().join(".ssh/id_ed25519")];
    let known_hosts = fixture.home.path().join(".ssh/known_hosts");
    if known {
        std::fs::write(
            &known_hosts,
            format!("[127.0.0.1]:{} {}\n", fixture.port, fixture.host_pub),
        )
        .unwrap();
    }
    target.known_hosts = Some(known_hosts);
    target
}

async fn connect(fixture: &Fixture, known: bool) -> nie_ssh::Result<Connection> {
    Connection::connect(&target(fixture, known), fixture.home.path(), HostKeyPolicy::Strict).await
}

#[tokio::test(flavor = "multi_thread")]
async fn exec_returns_streams_and_status() {
    let fixture = start(1, 1).await;
    let ssh = connect(&fixture, true).await.unwrap();
    let out = ssh.exec("printf hi; echo err >&2; exit 3").await.unwrap();
    assert_eq!(out.stdout_text(), "hi");
    assert_eq!(out.stderr_text(), "err\n");
    assert_eq!(out.status, Some(3));
    let out = ssh.exec("pwd").await.unwrap();
    let pwd = PathBuf::from(out.stdout_text().trim());
    assert_eq!(pwd.canonicalize().unwrap(), fixture.server_home.path().canonicalize().unwrap());
}

#[tokio::test(flavor = "multi_thread")]
async fn exec_feeds_stdin_and_survives_large_output() {
    let fixture = start(1, 1).await;
    let ssh = connect(&fixture, true).await.unwrap();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let payload: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();
    let status = ssh
        .exec_with("cat", std::io::Cursor::new(payload.clone()), &mut out, &mut err)
        .await
        .unwrap();
    assert_eq!(status, Some(0));
    assert_eq!(out, payload);
}

#[tokio::test(flavor = "multi_thread")]
async fn sftp_writes_lists_reads_and_removes() {
    use nie_ssh::russh_sftp::client::SftpSession;
    let fixture = start(1, 1).await;
    let ssh = connect(&fixture, true).await.unwrap();
    let sftp = SftpSession::new(ssh.open_subsystem("sftp").await.unwrap())
        .await
        .unwrap();
    sftp.create_dir("data").await.unwrap();
    let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 253) as u8).collect();
    let mut file = sftp.create("data/a.bin").await.unwrap();
    file.write_all(&payload).await.unwrap();
    file.close().await.unwrap();
    assert_eq!(std::fs::read(fixture.server_home.path().join("data/a.bin")).unwrap(), payload);
    assert_eq!(sftp.read("data/a.bin").await.unwrap(), payload);
    let names: Vec<String> = sftp.read_dir("data").await.unwrap().map(|e| e.file_name()).collect();
    assert_eq!(names, ["a.bin"]);
    assert_eq!(sftp.metadata("data/a.bin").await.unwrap().size, Some(200_000));
    sftp.remove_file("data/a.bin").await.unwrap();
    assert!(!Path::new(&fixture.server_home.path().join("data/a.bin")).exists());
    assert!(sftp.read("data/missing").await.is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn local_forward_reaches_a_loopback_service() {
    let fixture = start(1, 1).await;
    let echo = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let echo_port = echo.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = echo.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 64];
                while let Ok(n) = socket.read(&mut buf).await {
                    if n == 0 || socket.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });
    let ssh = Arc::new(connect(&fixture, true).await.unwrap());
    let forward = Forward::parse(&format!("127.0.0.1:0:127.0.0.1:{echo_port}")).unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    let forwarder = ssh.clone();
    tokio::spawn(async move {
        let _ = forwarder
            .forward_local(&forward, move |bound| {
                let _ = tx.send(bound);
            })
            .await;
    });
    let bound = rx.await.unwrap();
    let mut client = tokio::net::TcpStream::connect(bound).await.unwrap();
    client.write_all(b"through the tunnel").await.unwrap();
    let mut back = vec![0u8; 18];
    client.read_exact(&mut back).await.unwrap();
    assert_eq!(back, b"through the tunnel");
}

#[tokio::test(flavor = "multi_thread")]
async fn an_unlisted_key_is_refused() {
    let fixture = start(1, 2).await;
    let error = connect(&fixture, true).await.err().expect("must be refused");
    assert!(error.to_string().contains("authentication"), "{error}");
}

#[tokio::test(flavor = "multi_thread")]
async fn unknown_host_key_is_refused_unless_accept_new() {
    let fixture = start(1, 1).await;
    let error = connect(&fixture, false).await.err().expect("strict must refuse");
    assert!(error.to_string().contains("not in known_hosts"), "{error}");
    let ssh = Connection::connect(
        &target(&fixture, false),
        fixture.home.path(),
        HostKeyPolicy::AcceptNew,
    )
    .await
    .unwrap();
    assert_eq!(ssh.exec("echo ok").await.unwrap().stdout_text(), "ok\n");
    // The key was recorded: a strict connection now succeeds.
    assert!(connect(&fixture, false).await.is_ok());
    let recorded = std::fs::read_to_string(fixture.home.path().join(".ssh/known_hosts")).unwrap();
    assert!(recorded.contains(&fixture.host_pub[..20]));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_changed_host_key_is_refused() {
    let fixture = start(1, 1).await;
    let other = key(9).public_key().to_openssh().unwrap();
    std::fs::write(
        fixture.home.path().join(".ssh/known_hosts"),
        format!("[127.0.0.1]:{} {other}\n", fixture.port),
    )
    .unwrap();
    let mut target = target(&fixture, false);
    target.known_hosts = Some(fixture.home.path().join(".ssh/known_hosts"));
    let error = Connection::connect(&target, fixture.home.path(), HostKeyPolicy::AcceptNew)
        .await
        .err()
        .expect("changed key must be refused");
    assert!(error.to_string().contains("changed"), "{error}");
}
