//! `cargo xtask bindings-smoke`: smoke test of every generated binding against the cdylib of the contract.
//!
//! Each language runs when its toolchain exists; the summary names what ran and what was skipped, and the
//! task fails when a language that ran failed. The fixture is `tests/bindings/fixture.hex` decoded into a
//! temporary directory.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::util::{have, repo_root};

/// Arguments of `cargo xtask bindings-smoke`.
#[derive(clap::Args)]
pub struct Args {
    /// Path of libnie.so / nie.dll / libnie.dylib.
    #[arg(long, env = "NIE_LIB")]
    lib: PathBuf,
    /// Directory written by `nie bindgen` (default: `<repo>/bindings`).
    #[arg(long, env = "NIE_BINDINGS")]
    bindings: Option<PathBuf>,
    /// zig binary.
    #[arg(long, env = "NIE_ZIG", default_value = "zig")]
    zig: String,
}

/// Decodes the hex text of the fixture (whitespace ignored).
fn unhex(text: &str) -> Result<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        bail!("odd number of hex digits");
    }
    digits
        .chunks(2)
        .map(|pair| {
            let hi = (pair[0] as char).to_digit(16).context("hex digit")?;
            let lo = (pair[1] as char).to_digit(16).context("hex digit")?;
            Ok(u8::try_from(hi * 16 + lo)?)
        })
        .collect()
}

/// One language: its toolchain and the commands that build and run its smoke test, in order.
struct Step {
    name: &'static str,
    tool: String,
    commands: Vec<Command>,
}

fn path_str(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn steps(
    here: &Path,
    bindings: &Path,
    libdir: &Path,
    tmp: &Path,
    fixture: &Path,
    zig: &str,
) -> Result<Vec<Step>> {
    let b = |s: &str| bindings.join(s);
    let l = format!("-L{}", libdir.display());
    let fx = path_str(fixture);
    let cmd = |program: &str, args: &[String]| {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    let s = |v: &str| v.to_owned();

    let mut steps = vec![
        Step {
            name: "c",
            tool: s("cc"),
            commands: vec![
                cmd(
                    "cc",
                    &[
                        format!("-I{}", b("c").display()),
                        path_str(&here.join("smoke.c")),
                        l.clone(),
                        s("-lnie"),
                        s("-o"),
                        path_str(&tmp.join("c")),
                    ],
                ),
                cmd(&path_str(&tmp.join("c")), std::slice::from_ref(&fx)),
            ],
        },
        Step {
            name: "cpp",
            tool: s("c++"),
            commands: vec![
                cmd(
                    "c++",
                    &[
                        s("-std=c++20"),
                        format!("-I{}", b("cpp").display()),
                        path_str(&here.join("smoke.cpp")),
                        l.clone(),
                        s("-lnie"),
                        s("-o"),
                        path_str(&tmp.join("cpp")),
                    ],
                ),
                cmd(&path_str(&tmp.join("cpp")), std::slice::from_ref(&fx)),
            ],
        },
        Step {
            name: "rust",
            tool: s("rustc"),
            commands: vec![
                cmd(
                    "rustc",
                    &[
                        s("--edition"),
                        s("2024"),
                        s("-L"),
                        path_str(libdir),
                        s("-l"),
                        s("nie"),
                        path_str(&here.join("smoke_rs.rs")),
                        s("-o"),
                        path_str(&tmp.join("rs")),
                    ],
                ),
                cmd(&path_str(&tmp.join("rs")), std::slice::from_ref(&fx)),
            ],
        },
        Step {
            name: "python",
            tool: s("python3"),
            commands: vec![cmd(
                "python3",
                &[s("-I"), path_str(&here.join("smoke.py")), fx.clone()],
            )],
        },
        Step {
            name: "bun",
            tool: s("bun"),
            commands: vec![cmd("bun", &[path_str(&here.join("smoke.ts")), fx.clone()])],
        },
    ];

    let gomod = tmp.join("gomod");
    std::fs::create_dir_all(gomod.join("go"))?;
    std::fs::create_dir_all(gomod.join("c"))?;
    std::fs::copy(
        b("go").join("nie_contract.go"),
        gomod.join("go/nie_contract.go"),
    )?;
    std::fs::copy(
        here.join("go/smoke_test.go"),
        gomod.join("go/smoke_test.go"),
    )?;
    std::fs::copy(
        b("c").join("nie_contract.h"),
        gomod.join("c/nie_contract.h"),
    )?;
    std::fs::write(gomod.join("go/go.mod"), "module nie.local/smoke\ngo 1.24\n")?;
    let mut go = cmd("go", &[s("test"), s("-count=1"), s("-v"), s("./...")]);
    go.current_dir(gomod.join("go")).env("CGO_LDFLAGS", &l);
    steps.push(Step {
        name: "go",
        tool: s("go"),
        commands: vec![go],
    });

    let cs = tmp.join("cs");
    std::fs::create_dir_all(&cs)?;
    std::fs::copy(here.join("csharp/Program.cs"), cs.join("Program.cs"))?;
    let proj = std::fs::read_to_string(here.join("csharp/smoke.csproj"))?.replace(
        "../../../bindings/csharp/NieContract.cs",
        &path_str(&b("csharp").join("NieContract.cs")),
    );
    std::fs::write(cs.join("smoke.csproj"), proj)?;
    let mut dotnet = cmd(
        "dotnet",
        &[s("run"), s("-c"), s("Release"), s("--"), fx.clone()],
    );
    dotnet
        .current_dir(&cs)
        .env("DOTNET_NOLOGO", "1")
        .env("DOTNET_CLI_TELEMETRY_OPTOUT", "1");
    steps.push(Step {
        name: "csharp",
        tool: s("dotnet"),
        commands: vec![dotnet],
    });

    let zdir = tmp.join("zig");
    std::fs::create_dir_all(&zdir)?;
    std::fs::copy(here.join("zig/smoke.zig"), zdir.join("smoke.zig"))?;
    let mut build = cmd(
        zig,
        &[
            s("build-exe"),
            s("--dep"),
            s("nie_contract"),
            s("-Mroot=smoke.zig"),
            format!(
                "-Mnie_contract={}",
                b("zig").join("nie_contract.zig").display()
            ),
            s("-lc"),
            s("-lnie"),
            l,
            format!("-femit-bin={}", zdir.join("smoke").display()),
        ],
    );
    build.current_dir(&zdir);
    steps.push(Step {
        name: "zig",
        tool: zig.to_owned(),
        commands: vec![build, cmd(&path_str(&zdir.join("smoke")), &[])],
    });
    Ok(steps)
}

/// The last line of the output, or the whole tail on failure.
fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    all[all.len().saturating_sub(lines)..].join("\n")
}

/// Runs the commands in order; the output of the last one is returned, a failure carries its tail.
fn run_commands(
    commands: Vec<Command>,
    search: &str,
    env: &[(&str, &Path)],
) -> std::result::Result<String, String> {
    let mut last = String::new();
    for mut command in commands {
        command.env("LD_LIBRARY_PATH", search).envs(env.iter().copied());
        let out = command.output().map_err(|e| format!("{command:?}: {e}"))?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if !out.status.success() {
            return Err(tail(&text, 20));
        }
        last = text;
    }
    Ok(tail(&last, 1))
}

/// Runs the smoke test of every binding.
pub fn run(args: Args) -> Result<()> {
    let root = repo_root();
    let here = root.join("tests/bindings");
    let lib = args
        .lib
        .canonicalize()
        .with_context(|| format!("--lib {}", args.lib.display()))?;
    let libdir = lib.parent().context("library directory")?.to_path_buf();
    let bindings = args.bindings.unwrap_or_else(|| root.join("bindings"));
    let bindings = bindings
        .canonicalize()
        .with_context(|| format!("--bindings {}", bindings.display()))?;
    let tmp = std::env::temp_dir().join(format!("nie-bindings-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    let fixture = tmp.join("fixture.bin");
    std::fs::write(
        &fixture,
        unhex(&std::fs::read_to_string(here.join("fixture.hex"))?)?,
    )?;
    let inherited = std::env::var("LD_LIBRARY_PATH")
        .ok()
        .filter(|v| !v.is_empty());
    let search = match inherited {
        Some(rest) => format!("{}:{rest}", libdir.display()),
        None => libdir.display().to_string(),
    };

    // The smoke programs read the library, the generated bindings and the fixture from these variables.
    let env = [
        ("NIE_LIB", lib.as_path()),
        ("NIE_BINDINGS", bindings.as_path()),
        ("NIE_FIXTURE", fixture.as_path()),
    ];
    let built = steps(&here, &bindings, &libdir, &tmp, &fixture, &args.zig);
    let (mut ran, mut skipped, mut failed) = (Vec::new(), Vec::new(), Vec::new());
    for step in built? {
        if !have(&step.tool) {
            skipped.push(format!("{}({} missing)", step.name, step.tool));
            continue;
        }
        match run_commands(step.commands, &search, &env) {
            Ok(line) => {
                println!("{line}");
                ran.push(step.name);
            }
            Err(text) => {
                println!("FAIL {}\n{text}", step.name);
                failed.push(step.name);
            }
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
    println!("ran: {}", ran.join(" "));
    if !skipped.is_empty() {
        println!("skipped: {}", skipped.join(" "));
    }
    if !failed.is_empty() {
        bail!("failed: {}", failed.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_decodes_with_whitespace() {
        assert_eq!(unhex("00 ff\n10").unwrap(), [0x00, 0xff, 0x10]);
        assert!(unhex("abc").is_err());
        assert!(unhex("zz").is_err());
    }

    #[test]
    fn tails_keep_the_last_lines() {
        assert_eq!(tail("a\nb\nc", 1), "c");
        assert_eq!(tail("a\nb", 5), "a\nb");
    }

    #[test]
    fn a_failing_command_reports_its_output() {
        let mut c = Command::new("rustc");
        c.arg("--no-such-flag");
        if have("rustc") {
            assert!(run_commands(vec![c], "", &[]).is_err());
        }
    }
}
