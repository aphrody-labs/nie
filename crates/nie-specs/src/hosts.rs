use std::{
    collections::BTreeMap,
    fmt::Write as _,
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::{Host, Sources};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct HostSpec {
    pub name: String,
    pub os: String,
    pub kernel: String,
    pub kernel_version: String,
    pub distro: String,
    pub arch: String,
    pub cpu_model: String,
    pub cpu_vendor: String,
    pub logical_cpus: u32,
    pub cpu_flags: Vec<String>,
    pub memory_mib: u64,
    pub swap_mib: u64,
    pub hypervisor: String,
    pub kvm_device: bool,
    pub filesystems: Vec<Filesystem>,
    pub gpus: Vec<String>,
    pub nvidia: Vec<Nvidia>,
    pub libc: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Filesystem {
    pub mount: String,
    pub fstype: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Nvidia {
    pub name: String,
    pub driver: String,
    pub compute_cap: String,
    pub memory_mib: u64,
}

const LINUX_SCRIPT: &str = r#"
export LC_ALL=C
echo '##kernel'; uname -r
echo '##kernel_version'; uname -v
echo '##arch'; uname -m
echo '##distro'; (. /etc/os-release 2>/dev/null && echo "$PRETTY_NAME")
echo '##lscpu'; lscpu 2>/dev/null
echo '##flags'; grep -m1 -E '^(flags|Features)' /proc/cpuinfo
echo '##mem'; grep -E '^(MemTotal|SwapTotal):' /proc/meminfo
echo '##virt'; systemd-detect-virt 2>/dev/null
echo '##kvm'; [ -e /dev/kvm ] && echo yes || echo no
echo '##mounts'; findmnt -rn -o TARGET,FSTYPE 2>/dev/null
echo '##gpu'; lspci 2>/dev/null | grep -iE 'vga|3d controller|display controller'
echo '##nvidia'; nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv,noheader,nounits 2>/dev/null
echo '##libc'; ldd --version 2>&1 | head -1
"#;

const WINDOWS_SCRIPT: &str = r#"
$ErrorActionPreference = 'SilentlyContinue'
$os = Get-CimInstance Win32_OperatingSystem
$cpu = Get-CimInstance Win32_Processor | Select-Object -First 1
$cs = Get-CimInstance Win32_ComputerSystem
$gpus = @(Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name })
$vols = @(Get-Volume | Where-Object { $_.DriveLetter } | ForEach-Object { "$($_.DriveLetter):\|$($_.FileSystem)" })
$smi = @(& nvidia-smi --query-gpu=name,driver_version,compute_cap,memory.total --format=csv,noheader,nounits 2>$null)
[ordered]@{
  kernel = $os.Version; distro = $os.Caption; build = $os.BuildNumber; arch = $env:PROCESSOR_ARCHITECTURE
  cpu_model = $cpu.Name; cpu_vendor = $cpu.Manufacturer; logical = $cpu.NumberOfLogicalProcessors
  memory = $cs.TotalPhysicalMemory; hypervisor = $cs.HypervisorPresent
  gpus = $gpus; vols = $vols; nvidia = $smi
} | ConvertTo-Json -Compress
"#;

/// Collect one host: through ssh when it has an alias and is not the local machine.
pub fn collect(host: &Host, local: bool) -> Result<HostSpec> {
    let mut spec = match (host.os.as_str(), local, &host.ssh) {
        ("linux", true, _) => parse_linux(&host.name, &run_sh(Command::new("sh").arg("-s"))?)?,
        ("linux", false, Some(alias)) => {
            let out = nie_ssh::exec_blocking(alias, "sh -s", LINUX_SCRIPT.as_bytes().to_vec())
                .with_context(|| format!("ssh {alias}"))?;
            if out.status != Some(0) {
                bail!("collector on {alias} exited with {:?}", out.status);
            }
            parse_linux(&host.name, &out.stdout_text())?
        }
        ("windows", true, _) => collect_windows(&host.name)?,
        _ => bail!(
            "host {}: not reachable (no ssh alias and not local)",
            host.name
        ),
    };
    if local && spec.cpu_flags.is_empty() {
        spec.cpu_flags = local_isa_flags();
    }
    Ok(spec)
}

fn run_sh(cmd: &mut Command) -> Result<String> {
    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("spawn")?;
    child
        .stdin
        .take()
        .context("stdin")?
        .write_all(LINUX_SCRIPT.as_bytes())
        .context("write script")?;
    let out = child.wait_with_output().context("wait")?;
    if !out.status.success() {
        bail!("collector exited with {}", out.status);
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn sections(text: &str) -> BTreeMap<&str, Vec<&str>> {
    let mut map: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    let mut cur = "";
    for line in text.lines() {
        if let Some(name) = line.strip_prefix("##") {
            cur = name;
            map.entry(cur).or_default();
        } else if !line.trim().is_empty() {
            map.entry(cur).or_default().push(line.trim_end());
        }
    }
    map
}

fn first<'a>(m: &'a BTreeMap<&str, Vec<&str>>, k: &str) -> &'a str {
    m.get(k).and_then(|v| v.first()).copied().unwrap_or("")
}

fn kib_to_mib(lines: &[&str], key: &str) -> u64 {
    lines
        .iter()
        .find_map(|l| l.strip_prefix(key))
        .and_then(|r| r.trim_start_matches(':').split_whitespace().next())
        .and_then(|n| n.parse::<u64>().ok())
        .map_or(0, |k| k / 1024)
}

pub fn parse_linux(name: &str, text: &str) -> Result<HostSpec> {
    let m = sections(text);
    let lscpu: BTreeMap<&str, &str> = m
        .get("lscpu")
        .into_iter()
        .flatten()
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim(), v.trim())))
        .collect();
    let mem = m.get("mem").map(Vec::as_slice).unwrap_or(&[]);
    let mut flags: Vec<String> = first(&m, "flags")
        .split_once(':')
        .map(|(_, v)| v.split_whitespace().map(str::to_owned).collect())
        .unwrap_or_default();
    flags.sort();
    flags.dedup();
    let pseudo = [
        "proc",
        "sysfs",
        "cgroup",
        "cgroup2",
        "tmpfs",
        "devtmpfs",
        "devpts",
        "securityfs",
        "debugfs",
        "tracefs",
        "configfs",
        "fusectl",
        "mqueue",
        "hugetlbfs",
        "pstore",
        "bpf",
        "autofs",
        "binfmt_misc",
        "efivarfs",
        "overlay",
        "squashfs",
        "nsfs",
        "ramfs",
        "rpc_pipefs",
        "fuse.lxcfs",
    ];
    let skip_prefix = [
        "/proc",
        "/sys",
        "/dev",
        "/run",
        "/var/lib/docker",
        "/var/lib/containers",
        "/snap",
        "/var/snap",
    ];
    let mut filesystems: Vec<Filesystem> = m
        .get("mounts")
        .into_iter()
        .flatten()
        .filter_map(|l| l.split_once(' '))
        .filter(|(mnt, ty)| !pseudo.contains(ty) && !skip_prefix.iter().any(|p| mnt.starts_with(p)))
        .map(|(mnt, ty)| Filesystem {
            mount: mnt.to_owned(),
            fstype: ty.to_owned(),
        })
        .collect();
    filesystems.sort_by(|a, b| a.mount.cmp(&b.mount));
    filesystems.dedup_by(|a, b| a.mount == b.mount);
    let nvidia = m
        .get("nvidia")
        .into_iter()
        .flatten()
        .filter_map(|l| parse_nvidia(l))
        .collect();
    let spec = HostSpec {
        name: name.to_owned(),
        os: "linux".into(),
        kernel: first(&m, "kernel").to_owned(),
        kernel_version: first(&m, "kernel_version").to_owned(),
        distro: first(&m, "distro").to_owned(),
        arch: first(&m, "arch").to_owned(),
        cpu_model: lscpu.get("Model name").copied().unwrap_or("").to_owned(),
        cpu_vendor: lscpu.get("Vendor ID").copied().unwrap_or("").to_owned(),
        logical_cpus: lscpu
            .get("CPU(s)")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0),
        cpu_flags: flags,
        memory_mib: kib_to_mib(mem, "MemTotal"),
        swap_mib: kib_to_mib(mem, "SwapTotal"),
        hypervisor: first(&m, "virt").to_owned(),
        kvm_device: first(&m, "kvm") == "yes",
        filesystems,
        gpus: m
            .get("gpu")
            .map(|v| v.iter().map(|s| (*s).to_owned()).collect())
            .unwrap_or_default(),
        nvidia,
        libc: first(&m, "libc").to_owned(),
    };
    if spec.kernel.is_empty() {
        bail!("host {name}: empty collector output");
    }
    Ok(spec)
}

fn parse_nvidia(line: &str) -> Option<Nvidia> {
    let p: Vec<&str> = line.split(',').map(str::trim).collect();
    let [name, driver, cc, mem] = p.as_slice() else {
        return None;
    };
    Some(Nvidia {
        name: (*name).into(),
        driver: (*driver).into(),
        compute_cap: (*cc).into(),
        memory_mib: mem.parse().ok()?,
    })
}

fn collect_windows(name: &str) -> Result<HostSpec> {
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", WINDOWS_SCRIPT])
        .output()
        .context("powershell")?;
    if !out.status.success() {
        bail!("powershell exited with {}", out.status);
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).context("powershell json")?;
    let strs = |k: &str| -> Vec<String> {
        match &v[k] {
            serde_json::Value::Array(a) => a
                .iter()
                .filter_map(|x| x.as_str().map(str::to_owned))
                .collect(),
            serde_json::Value::String(s) => vec![s.clone()],
            _ => Vec::new(),
        }
    };
    let text = |k: &str| v[k].as_str().unwrap_or("").to_owned();
    let mut filesystems: Vec<Filesystem> = strs("vols")
        .iter()
        .filter_map(|s| s.split_once('|'))
        .map(|(m, t)| Filesystem {
            mount: m.to_owned(),
            fstype: t.to_owned(),
        })
        .collect();
    filesystems.sort_by(|a, b| a.mount.cmp(&b.mount));
    Ok(HostSpec {
        name: name.to_owned(),
        os: "windows".into(),
        kernel: text("kernel"),
        kernel_version: format!("build {}", text("build")),
        distro: text("distro"),
        arch: text("arch"),
        cpu_model: text("cpu_model"),
        cpu_vendor: text("cpu_vendor"),
        logical_cpus: v["logical"].as_u64().unwrap_or(0) as u32,
        cpu_flags: Vec::new(),
        memory_mib: v["memory"].as_u64().unwrap_or(0) / (1024 * 1024),
        swap_mib: 0,
        hypervisor: if v["hypervisor"].as_bool().unwrap_or(false) {
            "present".into()
        } else {
            "none".into()
        },
        kvm_device: false,
        filesystems,
        gpus: strs("gpus"),
        nvidia: strs("nvidia")
            .iter()
            .filter_map(|l| parse_nvidia(l))
            .collect(),
        libc: String::new(),
    })
}

/// ISA extensions the running binary can see, for the machine that executes it.
fn local_isa_flags() -> Vec<String> {
    #[cfg(target_arch = "x86_64")]
    {
        macro_rules! probe {
            ($($f:tt),*) => {{
                let mut v = Vec::new();
                $( if std::arch::is_x86_feature_detected!($f) { v.push($f.to_owned()); } )*
                v
            }};
        }
        probe!(
            "sse4.2",
            "avx",
            "avx2",
            "fma",
            "bmi1",
            "bmi2",
            "popcnt",
            "aes",
            "pclmulqdq",
            "sha",
            "avx512f",
            "avx512bw",
            "avx512vl",
            "avx512vnni",
            "rdrand",
            "rdseed",
            "adx",
            "f16c",
            "lzcnt",
            "movbe"
        )
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        Vec::new()
    }
}

/// `a`, `b` and `c` in code spans.
fn name_list(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|n| format!("`{n}`")).collect();
    match quoted.split_last() {
        Some((last, head)) if !head.is_empty() => format!("{} and {last}", head.join(", ")),
        Some((last, _)) => last.clone(),
        None => String::new(),
    }
}

/// `hosts.md`; `remote` names the hosts collected over ssh.
pub fn render_hosts(specs: &[HostSpec], remote: &[&str]) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "<!-- Generated by `nie-specs hosts`; do not edit. -->\n");
    let _ = writeln!(s, "# Host specs\n");
    let _ = writeln!(
        s,
        "Collected live: {} over ssh (refreshed by the timer in `crates/nie-specs/deploy`), the local machine \
         on demand with `nie-specs hosts --local-as <name>`. Raw data: `hosts/<name>.json`.\n",
        name_list(remote)
    );
    let _ = writeln!(
        s,
        "| Host | OS | Kernel | CPU | vCPU | RAM MiB | Virtualization | GPU | Root fs |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|");
    for h in specs {
        let gpu = if h.nvidia.is_empty() {
            if h.gpus.is_empty() {
                "none".to_owned()
            } else {
                h.gpus.join("; ")
            }
        } else {
            h.nvidia
                .iter()
                .map(|n| {
                    format!(
                        "{} {} MiB, driver {}, cc {}",
                        n.name, n.memory_mib, n.driver, n.compute_cap
                    )
                })
                .collect::<Vec<_>>()
                .join("; ")
        };
        let root = h
            .filesystems
            .iter()
            .find(|f| f.mount == "/" || f.mount == "C:\\")
            .map_or("-", |f| f.fstype.as_str());
        let virt = if h.hypervisor.is_empty() {
            "-"
        } else {
            &h.hypervisor
        };
        let _ = writeln!(
            s,
            "| {} | {} | {} | {} | {} | {} | {}{} | {} | {} |",
            h.name,
            h.distro,
            h.kernel,
            h.cpu_model,
            h.logical_cpus,
            h.memory_mib,
            virt,
            if h.kvm_device { ", /dev/kvm" } else { "" },
            gpu.replace('|', "/"),
            root
        );
    }
    for h in specs {
        let _ = writeln!(s, "\n## {}\n", h.name);
        let _ = writeln!(s, "- Kernel: `{}` ({})", h.kernel, h.kernel_version);
        let _ = writeln!(s, "- Architecture: {}; CPU vendor {}", h.arch, h.cpu_vendor);
        if !h.libc.is_empty() {
            let _ = writeln!(s, "- libc: {}", h.libc);
        }
        let _ = writeln!(s, "- Swap: {} MiB", h.swap_mib);
        let fs = h
            .filesystems
            .iter()
            .map(|f| format!("`{}` {}", f.mount, f.fstype))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(s, "- Filesystems: {fs}");
        if !h.cpu_flags.is_empty() {
            let _ = writeln!(
                s,
                "- ISA/CPU flags ({}): {}",
                h.cpu_flags.len(),
                h.cpu_flags.join(" ")
            );
        }
    }
    s
}

/// Collect and write `hosts/<name>.json` plus `hosts.md`; returns the changed files.
pub fn run(root: &Path, names: &[String], local_as: Option<&str>) -> Result<Vec<(String, String)>> {
    let sources = Sources::load(&root.join("sources.toml"))?;
    let dir = root.join("hosts");
    std::fs::create_dir_all(&dir)?;
    let mut failures = Vec::new();
    let wanted: Vec<&Host> = sources
        .host
        .iter()
        .filter(|h| {
            if names.is_empty() {
                h.ssh.is_some() || Some(h.name.as_str()) == local_as
            } else {
                names.contains(&h.name) || Some(h.name.as_str()) == local_as
            }
        })
        .collect();
    for host in wanted {
        let local = Some(host.name.as_str()) == local_as;
        match collect(host, local) {
            Ok(spec) => {
                let json = serde_json::to_string_pretty(&spec)? + "\n";
                std::fs::write(dir.join(format!("{}.json", host.name)), json)?;
                println!("collected {}", host.name);
            }
            Err(e) => failures.push(format!("{}: {e:#}", host.name)),
        }
    }
    // Rebuild hosts.md from every JSON present, in sources.toml order.
    let mut specs = Vec::new();
    for h in &sources.host {
        if let Ok(t) = std::fs::read_to_string(dir.join(format!("{}.json", h.name))) {
            specs.push(serde_json::from_str::<HostSpec>(&t)?);
        }
    }
    let remote: Vec<&str> = sources
        .host
        .iter()
        .filter(|h| h.ssh.is_some())
        .map(|h| h.name.as_str())
        .collect();
    std::fs::write(root.join("hosts.md"), render_hosts(&specs, &remote))?;
    for f in &failures {
        eprintln!("FAIL {f}");
    }
    let mut files = vec![(
        "hosts.md".to_owned(),
        std::fs::read_to_string(root.join("hosts.md"))?,
    )];
    for h in &sources.host {
        let rel = format!("hosts/{}.json", h.name);
        if let Ok(t) = std::fs::read_to_string(root.join(&rel)) {
            files.push((rel, t));
        }
    }
    if !failures.is_empty() {
        bail!("{} host(s) failed", failures.len());
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_collector_output_parses() {
        let text = "##kernel\n7.3.0-10-generic\n##kernel_version\n#10-Ubuntu SMP\n##arch\nx86_64\n##distro\nUbuntu 26.04\n\
##lscpu\nArchitecture: x86_64\nCPU(s): 12\nModel name: Intel Haswell\nVendor ID: GenuineIntel\n##flags\nflags : fpu sse avx2 avx2\n\
##mem\nMemTotal: 47185920 kB\nSwapTotal: 0 kB\n##virt\nkvm\n##kvm\nyes\n##mounts\n/ ext4\n/proc proc\n/srv ext4\n\
##gpu\n00:02.0 VGA compatible controller: Cirrus Logic\n##nvidia\n##libc\nldd (Ubuntu GLIBC 2.43) 2.43\n";
        let h = parse_linux("fr1", text).unwrap();
        assert_eq!((h.logical_cpus, h.memory_mib), (12, 46080));
        assert_eq!(h.cpu_flags, ["avx2", "fpu", "sse"]);
        assert_eq!(h.filesystems.len(), 2);
        assert!(h.kvm_device && h.nvidia.is_empty());
    }

    #[test]
    fn nvidia_line_parses() {
        let n = parse_nvidia("NVIDIA GeForce RTX 4070, 621.14, 8.9, 12282").unwrap();
        assert_eq!((n.compute_cap.as_str(), n.memory_mib), ("8.9", 12282));
    }
}
