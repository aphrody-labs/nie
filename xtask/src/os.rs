//! `cargo xtask os`: the QEMU arguments, the reproducible initramfs and the boot of the Linux-ABI
//! Asterinas kernel with nie inside.
//!
//! Everything is derived from the host and from `NIE_OS_*` variables:
//!
//! - `NIE_OS_CPU`  guest CPU model (default `EPYC-Genoa`, the prd EPYC 9354P family)
//! - `NIE_OS_SMP`  guest CPUs (default 8)
//! - `NIE_OS_MEM`  guest memory (default 8G)
//! - `NIE_OS_PORT` host TCP port forwarded to guest 8790 (default 8790)
//! - `NIE_OS_FS`   virtiofsd socket; adds the vhost-user-fs device tagged `nie-game`
//! - `NIE_OS_DISK` raw image attached as virtio-blk
//! - `NIE_OS_LOG`  serial log file (default `qemu-serial.log`)
//! - `NIE_OS_STATE` private copy of the OVMF variable store (default a temporary file)

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use sha2::{Digest, Sha256};

use crate::util::{hex, repo_root};

#[derive(clap::Args)]
pub struct Args {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand)]
pub enum Cmd {
    /// Print the QEMU argument line for `cargo osdk` (OSDK.toml `args`).
    Qemu,
    /// Pack a reproducible initramfs: static busybox, the static musl `nie` and os/initramfs/init.
    Initramfs {
        /// Static `nie` binary.
        nie: PathBuf,
        /// Output `.cpio.gz`.
        out: PathBuf,
        /// busybox binary (default: `busybox-static`, else `busybox`, from PATH).
        busybox: Option<PathBuf>,
    },
    /// Boot the Linux-ABI kernel of vendor/asterinas with nie in the initramfs and the game directory over virtiofs.
    Boot {
        /// Game directory shared read-only as the virtiofs tag `nie-game`.
        game_dir: PathBuf,
        /// Static `nie` binary.
        nie: PathBuf,
    },
}

pub fn run(args: Args) -> Result<()> {
    match args.cmd {
        Cmd::Qemu => {
            println!("{}", qemu_args(&|name| std::env::var(name).ok())?);
            Ok(())
        }
        Cmd::Initramfs { nie, out, busybox } => {
            let digest = initramfs(&nie, &out, busybox.as_deref())?;
            println!("{digest}  {}", out.display());
            Ok(())
        }
        Cmd::Boot { game_dir, nie } => boot(&game_dir, &nie),
    }
}

fn first_existing(paths: &[&str]) -> Option<String> {
    paths
        .iter()
        .find(|p| Path::new(p).exists())
        .map(|p| (*p).to_owned())
}

/// The QEMU argument line; `get` reads the `NIE_OS_*` settings (an injected lookup keeps it testable).
pub fn qemu_args(get: &dyn Fn(&str) -> Option<String>) -> Result<String> {
    let or = |name: &str, default: &str| {
        get(name)
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| default.to_owned())
    };
    let mem = or("NIE_OS_MEM", "8G");
    let mut args = format!(
        "-machine q35,kernel-irqchip=split -cpu {},+x2apic -smp {} -m {mem}",
        or("NIE_OS_CPU", "EPYC-Genoa"),
        or("NIE_OS_SMP", "8")
    );
    if std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/kvm")
        .is_ok()
    {
        args.push_str(" -accel kvm");
    }
    args.push_str(" --no-reboot -nographic -display none -serial chardev:mux -monitor chardev:mux");
    args.push_str(&format!(
        " -chardev stdio,id=mux,mux=on,signal=off,logfile={}",
        or("NIE_OS_LOG", "qemu-serial.log")
    ));
    args.push_str(" -device isa-debug-exit,iobase=0xf4,iosize=0x04");
    args.push_str(&format!(
        " -netdev user,id=net01,hostfwd=tcp::{}-:8790",
        or("NIE_OS_PORT", "8790")
    ));
    args.push_str(" -device virtio-net-pci,netdev=net01,disable-legacy=on,disable-modern=off");
    args.push_str(
        " -object rng-random,id=rng0,filename=/dev/urandom -device virtio-rng-pci,rng=rng0,disable-legacy=on,disable-modern=off",
    );
    if let Some(disk) = get("NIE_OS_DISK").filter(|v| !v.is_empty()) {
        args.push_str(&format!(" -drive if=none,format=raw,id=disk0,file={disk}"));
        args.push_str(" -device virtio-blk-pci,drive=disk0,disable-legacy=on,disable-modern=off");
    }
    if let Some(fs) = get("NIE_OS_FS").filter(|v| !v.is_empty()) {
        args.push_str(&format!(
            " -object memory-backend-memfd,id=mem0,size={mem},share=on -numa node,memdev=mem0"
        ));
        args.push_str(&format!(
            " -chardev socket,id=fs0,path={fs} -device vhost-user-fs-pci,chardev=fs0,tag=nie-game"
        ));
    }
    // grub-rescue-iso boots through UEFI: firmware code and a private copy of the variable store.
    let code = first_existing(&[
        "/usr/share/OVMF/OVMF_CODE_4M.fd",
        "/usr/share/OVMF/OVMF_CODE.fd",
        "/usr/share/edk2/x64/OVMF_CODE.4m.fd",
    ]);
    let vars = first_existing(&[
        "/usr/share/OVMF/OVMF_VARS_4M.fd",
        "/usr/share/OVMF/OVMF_VARS.fd",
        "/usr/share/edk2/x64/OVMF_VARS.4m.fd",
    ]);
    if let (Some(code), Some(vars)) = (code, vars) {
        let state = get("NIE_OS_STATE")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| {
                let tmp = get("TMPDIR")
                    .filter(|v| !v.is_empty())
                    .unwrap_or_else(|| "/tmp".to_owned());
                format!("{tmp}/nie-os-ovmf-vars-{}.fd", std::process::id())
            });
        std::fs::copy(&vars, &state).with_context(|| format!("copy {vars} to {state}"))?;
        args.push_str(&format!(
            " -drive if=pflash,format=raw,unit=0,readonly=on,file={code} -drive if=pflash,format=raw,unit=1,file={state}"
        ));
    }
    Ok(args)
}

/// First of `names` found on `PATH`.
pub fn find_in_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    names.iter().find_map(|name| {
        std::env::split_paths(&path)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}

/// `chmod` on unix, nothing elsewhere.
pub fn chmod(path: &Path, mode: u32) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
            .with_context(|| format!("chmod {mode:o} {}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = (path, mode);
    Ok(())
}

/// `ln -sf target link`.
pub fn symlink(target: &str, link: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = std::fs::remove_file(link);
        std::os::unix::fs::symlink(target, link)
            .with_context(|| format!("symlink {}", link.display()))
    }
    #[cfg(not(unix))]
    {
        let _ = (target, link);
        bail!("symlinks of an initramfs need a unix host")
    }
}

/// A `newc` cpio archive of the tree under `root`: entries sorted bytewise by path, owner 0:0, mtime 0,
/// inode numbers in order, so the same tree gives the same bytes.
pub fn cpio_newc(root: &Path) -> Result<Vec<u8>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;

        fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, PathBuf)>) -> Result<()> {
            for entry in std::fs::read_dir(dir).with_context(|| dir.display().to_string())? {
                let path = entry?.path();
                let rel = path
                    .strip_prefix(root)?
                    .to_string_lossy()
                    .replace('\\', "/");
                let is_dir = std::fs::symlink_metadata(&path)?.is_dir();
                out.push((rel, path.clone()));
                if is_dir {
                    collect(root, &path, out)?;
                }
            }
            Ok(())
        }
        fn pad(out: &mut Vec<u8>) {
            while !out.len().is_multiple_of(4) {
                out.push(0);
            }
        }
        fn entry(out: &mut Vec<u8>, ino: u32, mode: u32, size: u32, name: &str) {
            let fields = [
                ino,
                mode,
                0,
                0,
                1,
                0,
                size,
                0,
                0,
                0,
                0,
                name.len() as u32 + 1,
                0,
            ];
            out.extend_from_slice(b"070701");
            for field in fields {
                out.extend_from_slice(format!("{field:08X}").as_bytes());
            }
            out.extend_from_slice(name.as_bytes());
            out.push(0);
            pad(out);
        }

        let mut entries = Vec::new();
        collect(root, root, &mut entries)?;
        entries.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
        let mut out = Vec::new();
        for (index, (name, path)) in entries.iter().enumerate() {
            let meta = std::fs::symlink_metadata(path)?;
            let data = if meta.is_symlink() {
                std::fs::read_link(path)?
                    .to_string_lossy()
                    .into_owned()
                    .into_bytes()
            } else if meta.is_file() {
                std::fs::read(path)?
            } else {
                Vec::new()
            };
            entry(
                &mut out,
                index as u32 + 1,
                meta.mode(),
                u32::try_from(data.len())?,
                name,
            );
            out.extend_from_slice(&data);
            pad(&mut out);
        }
        entry(&mut out, 0, 0, 0, "TRAILER!!!");
        Ok(out)
    }
    #[cfg(not(unix))]
    {
        let _ = root;
        bail!("cpio archives of a tree need a unix host")
    }
}

/// `gzip -<level>n`: no name, no timestamp.
pub fn gzip(data: &[u8], level: u8) -> Result<Vec<u8>> {
    let mut child = Command::new("gzip")
        .arg(format!("-{level}n"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .context("spawn gzip")?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("gzip stdin"))?;
    let out = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(data));
        let out = child.wait_with_output();
        let written = writer.join().map_err(|_| anyhow!("gzip writer panicked"));
        (out, written)
    });
    let (output, written) = out;
    written??;
    let output = output.context("wait for gzip")?;
    if !output.status.success() {
        bail!("gzip exited with {}", output.status);
    }
    Ok(output.stdout)
}

/// Removes a directory tree when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Packs the initramfs and returns the SHA-256 of the written file.
fn initramfs(nie: &Path, out: &Path, busybox: Option<&Path>) -> Result<String> {
    let busybox = match busybox {
        Some(path) => path.to_path_buf(),
        None => find_in_path(&["busybox-static", "busybox"])
            .context("no busybox-static or busybox on PATH")?,
    };
    let init = repo_root().join("os/initramfs/init");
    let stage = Scratch::new("nie-initramfs")?;
    let root = &stage.0;
    for dir in ["bin", "proc", "sys", "dev", "tmp", "game"] {
        std::fs::create_dir_all(root.join(dir))?;
    }
    for (from, to) in [
        (busybox.as_path(), "bin/busybox"),
        (nie, "bin/nie"),
        (init.as_path(), "init"),
    ] {
        std::fs::copy(from, root.join(to)).with_context(|| format!("copy {}", from.display()))?;
        chmod(&root.join(to), 0o755)?;
    }
    for applet in [
        "sh",
        "mount",
        "grep",
        "ls",
        "tr",
        "mkdir",
        "poweroff",
        "cat",
        "ip",
        "ifconfig",
        "ps",
        "nc",
        "sha256sum",
    ] {
        symlink("busybox", &root.join("bin").join(applet))?;
    }
    let packed = gzip(&cpio_newc(root)?, 9)?;
    std::fs::write(out, &packed).with_context(|| out.display().to_string())?;
    Ok(hex(&Sha256::digest(&packed)))
}

/// Boots the Linux-ABI kernel of `vendor/asterinas` under QEMU/KVM with nie in the initramfs and the
/// game directory shared read-only through virtiofs (tag `nie-game`, mounted on `/game`).
///
/// `ASTERINAS` names another checkout, `OSDK_BIN` the directory of the `cargo-osdk` built from it.
fn boot(game_dir: &Path, nie: &Path) -> Result<()> {
    let game = game_dir
        .canonicalize()
        .with_context(|| format!("game directory {}", game_dir.display()))?;
    let nie = nie
        .canonicalize()
        .with_context(|| format!("nie binary {}", nie.display()))?;
    let asterinas = match std::env::var_os("ASTERINAS") {
        Some(path) => PathBuf::from(path),
        None => repo_root().join("vendor/asterinas"),
    }
    .canonicalize()
    .context("the Asterinas checkout (set ASTERINAS)")?;
    let work = Scratch::new("nie-os-boot")?;
    let initramfs_path = work.0.join("initramfs.cpio.gz");
    initramfs(&nie, &initramfs_path, None)?;
    let socket = work.0.join("virtiofsd.sock");
    let socket_text = socket.to_string_lossy().into_owned();
    let args = qemu_args(&|name| match name {
        "NIE_OS_FS" => Some(socket_text.clone()),
        other => std::env::var(other).ok(),
    })?;
    let mut cmd = Command::new("cargo");
    cmd.current_dir(asterinas.join("kernel"))
        .args(["osdk", "run", "--target-arch", "x86_64", "--release", "--initramfs"])
        .arg(&initramfs_path)
        .arg("--config")
        .arg(format!("qemu.args={args}"))
        .arg("--qemu-with-daemon")
        .arg(format!(
            "virtiofsd --socket-path={socket_text} --shared-dir={} --readonly --sandbox none --cache auto",
            game.display()
        ));
    if let Some(bin) = std::env::var_os("OSDK_BIN") {
        let mut paths = vec![PathBuf::from(bin)];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        cmd.env("PATH", std::env::join_paths(paths)?);
    }
    crate::util::run(&mut cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| (*v).to_owned())
        }
    }

    #[test]
    fn defaults_and_overrides_of_the_qemu_line() {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("vars.fd").to_string_lossy().into_owned();
        let line = qemu_args(&env(&[("NIE_OS_STATE", &state)])).unwrap();
        assert!(
            line.starts_with(
                "-machine q35,kernel-irqchip=split -cpu EPYC-Genoa,+x2apic -smp 8 -m 8G"
            )
        );
        assert!(line.contains("logfile=qemu-serial.log"));
        assert!(line.contains("hostfwd=tcp::8790-:8790"));
        assert!(!line.contains("virtio-blk"));
        let line = qemu_args(&env(&[
            ("NIE_OS_CPU", "host"),
            ("NIE_OS_SMP", "2"),
            ("NIE_OS_MEM", "1G"),
            ("NIE_OS_PORT", "9000"),
            ("NIE_OS_DISK", "/d.img"),
            ("NIE_OS_FS", "/s.sock"),
            ("NIE_OS_STATE", &state),
        ]))
        .unwrap();
        assert!(line.contains("-cpu host,+x2apic -smp 2 -m 1G"));
        assert!(line.contains("hostfwd=tcp::9000-:8790"));
        assert!(line.contains("file=/d.img"));
        assert!(line.contains("size=1G,share=on"));
        assert!(line.contains("path=/s.sock"));
        assert!(line.contains("tag=nie-game"));
    }

    #[cfg(unix)]
    #[test]
    fn the_cpio_archive_is_sorted_and_reproducible() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("bin")).unwrap();
        std::fs::write(dir.path().join("bin/b"), b"abc").unwrap();
        std::fs::write(dir.path().join("init"), b"#!/bin/sh\n").unwrap();
        symlink("b", &dir.path().join("bin/link")).unwrap();
        let one = cpio_newc(dir.path()).unwrap();
        assert_eq!(one, cpio_newc(dir.path()).unwrap());
        assert_eq!(one.len() % 4, 0);
        assert!(one.starts_with(b"070701"));
        let text = String::from_utf8_lossy(&one).into_owned();
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle}"));
        assert!(at("bin\0") < at("bin/b\0"));
        assert!(at("bin/b\0") < at("bin/link\0"));
        assert!(at("bin/link\0") < at("init\0"));
        assert!(text.contains("TRAILER!!!"));
    }
}
