//! One `Step` type and one runner for every dev command.
//!
//! A step is a command (or an in-process function) with a place: `Local` steps always run here,
//! `Build` steps run on the build host when it is reachable and here otherwise. The runner syncs
//! the sources once, runs every step (a failure does not stop the others) and renders one compact
//! report.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Instant;

use anyhow::{Context, Result, bail};

use super::host::{BuildHost, Cmd};

/// Where a step is allowed to run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Place {
    /// Always on this machine.
    Local,
    /// On the build host when available, here otherwise.
    Build,
}

/// How the caller wants heavy steps placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Remote when the host answers, local with a note otherwise.
    #[default]
    Auto,
    /// Everything on this machine.
    Local,
    /// Heavy steps on the build host, failing when it is unreachable.
    Remote,
}

/// The `--local` / `--remote` pair shared by the heavy commands.
#[derive(clap::Args, Debug, Clone, Copy, Default)]
pub struct Exec {
    /// Run every step on this machine.
    #[arg(long, conflicts_with = "remote")]
    pub local: bool,
    /// Run heavy steps on the build host and fail when it is unreachable.
    #[arg(long)]
    pub remote: bool,
}

impl Exec {
    /// The placement mode these flags select.
    #[must_use]
    pub fn mode(self) -> Mode {
        match (self.local, self.remote) {
            (true, _) => Mode::Local,
            (_, true) => Mode::Remote,
            _ => Mode::Auto,
        }
    }
}

/// An in-process action.
pub type Call = Box<dyn FnOnce() -> Result<()>>;
/// Receives the captured standard output of a step.
pub type Sink = Box<dyn FnOnce(&str) -> Result<()>>;

/// What a step does.
pub enum Action {
    /// Runs a command.
    Run(Cmd),
    /// Calls a function.
    Call(Call),
}

/// One unit of work.
pub struct Step {
    /// Name shown in the report.
    pub name: String,
    /// Placement.
    pub place: Place,
    /// The action (on the build host when remote).
    pub action: Action,
    /// Replacement action used when the step runs here.
    pub local: Option<Action>,
    /// Receives the standard output instead of showing it.
    pub sink: Option<Sink>,
}

impl Step {
    /// A command that runs wherever the mode puts it.
    #[must_use]
    pub fn build(name: &str, cmd: Cmd) -> Self {
        Self {
            name: name.to_owned(),
            place: Place::Build,
            action: Action::Run(cmd),
            local: None,
            sink: None,
        }
    }

    /// A command that always runs here.
    #[must_use]
    pub fn local(name: &str, cmd: Cmd) -> Self {
        Self {
            place: Place::Local,
            ..Self::build(name, cmd)
        }
    }

    /// A function that always runs here.
    #[must_use]
    pub fn call(name: &str, call: impl FnOnce() -> Result<()> + 'static) -> Self {
        Self {
            name: name.to_owned(),
            place: Place::Local,
            action: Action::Call(Box::new(call)),
            local: None,
            sink: None,
        }
    }

    /// Uses another action when the step runs here.
    #[must_use]
    pub fn or_locally(mut self, action: Action) -> Self {
        self.local = Some(action);
        self
    }

    /// Captures the standard output and hands it to `sink`.
    #[must_use]
    pub fn captured(mut self, sink: impl FnOnce(&str) -> Result<()> + 'static) -> Self {
        self.sink = Some(Box::new(sink));
        self
    }
}

/// How a step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Succeeded.
    Ok,
    /// Failed, with the reason.
    Failed(String),
}

/// The outcome of one step.
#[derive(Debug, Clone)]
pub struct StepResult {
    /// Step name.
    pub name: String,
    /// Where it ran.
    pub place: String,
    /// Outcome.
    pub status: Status,
    /// Wall time in seconds.
    pub secs: f64,
}

/// The results of a run.
#[derive(Debug, Clone, Default)]
pub struct Report {
    /// One entry per step, in order.
    pub results: Vec<StepResult>,
}

impl Report {
    /// The compact table printed at the end of a command.
    #[must_use]
    pub fn render(&self) -> String {
        let width = self
            .results
            .iter()
            .map(|r| r.name.len())
            .max()
            .unwrap_or(4)
            .max(4);
        let mut out = format!(
            "\n{:<width$}  {:<8}  {:<6}  {:>8}\n",
            "step", "where", "status", "secs"
        );
        for r in &self.results {
            let status = if r.status == Status::Ok {
                "ok"
            } else {
                "FAILED"
            };
            let _ = writeln!(
                out,
                "{:<width$}  {:<8}  {:<6}  {:>8.1}",
                r.name, r.place, status, r.secs
            );
        }
        for r in &self.results {
            if let Status::Failed(why) = &r.status {
                let _ = writeln!(out, "  {}: {why}", r.name);
            }
        }
        out
    }

    /// `Ok` when every step succeeded, an error naming the failed steps otherwise.
    ///
    /// # Errors
    /// When at least one step failed.
    pub fn into_result(self) -> Result<()> {
        let failed: Vec<&str> = self
            .results
            .iter()
            .filter(|r| r.status != Status::Ok)
            .map(|r| r.name.as_str())
            .collect();
        if failed.is_empty() {
            Ok(())
        } else {
            bail!("failed: {}", failed.join(", "))
        }
    }
}

pub use crate::util::repo_root;

/// Runs steps and decides where they run.
pub struct Runner {
    host: BuildHost,
    mode: Mode,
    root: PathBuf,
    remote: Option<bool>,
    synced: Option<std::result::Result<usize, String>>,
}

impl Runner {
    /// A runner for the repository containing the current directory.
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        let root = repo_root();
        Self::with(BuildHost::from_env(&root), mode, root)
    }

    /// A runner with explicit settings.
    #[must_use]
    pub fn with(host: BuildHost, mode: Mode, root: PathBuf) -> Self {
        Self {
            host,
            mode,
            root,
            remote: None,
            synced: None,
        }
    }

    /// The build host.
    #[must_use]
    pub fn host(&self) -> &BuildHost {
        &self.host
    }

    /// The workspace root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn remote(&mut self) -> bool {
        if let Some(known) = self.remote {
            return known;
        }
        let remote = match self.mode {
            Mode::Local => false,
            Mode::Remote => true,
            Mode::Auto if self.host.is_here() => false,
            Mode::Auto => {
                let up = self.host.reachable();
                if !up {
                    eprintln!(
                        "note: build host `{}` is unreachable, heavy steps run on this machine",
                        self.host.name
                    );
                }
                up
            }
        };
        self.remote = Some(remote);
        remote
    }

    fn ensure_synced(&mut self) -> Result<()> {
        if self.synced.is_none() {
            eprintln!("syncing sources to {}:{}", self.host.name, self.host.tree());
            let result = self.host.sync(&self.root).map_err(|e| format!("{e:#}"));
            if let Ok(count) = &result {
                eprintln!("synced {count} files");
            }
            self.synced = Some(result);
        }
        match self.synced.as_ref() {
            Some(Err(why)) => bail!("sync failed: {why}"),
            _ => Ok(()),
        }
    }

    /// Runs every step and returns the report.
    pub fn run(&mut self, steps: Vec<Step>) -> Report {
        let mut report = Report::default();
        for step in steps {
            let remote = step.place == Place::Build && self.remote();
            let place = if remote {
                self.host.name.clone()
            } else {
                "local".into()
            };
            eprintln!("== {} ({place})", step.name);
            let started = Instant::now();
            let outcome = self.execute(step.action, step.local, step.sink, remote);
            let status = match outcome {
                Ok(()) => Status::Ok,
                Err(e) => Status::Failed(format!("{e:#}")),
            };
            report.results.push(StepResult {
                name: step.name,
                place,
                status,
                secs: started.elapsed().as_secs_f64(),
            });
        }
        report
    }

    fn execute(
        &mut self,
        action: Action,
        local: Option<Action>,
        sink: Option<Sink>,
        remote: bool,
    ) -> Result<()> {
        let action = if remote {
            action
        } else {
            local.unwrap_or(action)
        };
        match action {
            Action::Call(call) => call(),
            Action::Run(cmd) => {
                let capture = sink.is_some();
                let mut process = if remote {
                    self.ensure_synced()?;
                    let mut p = Command::new("ssh");
                    p.args(self.host.ssh_args(&self.host.script(&cmd)));
                    p
                } else {
                    let mut p = Command::new(&cmd.program);
                    p.args(&cmd.args)
                        .envs(cmd.env.iter().cloned())
                        .current_dir(&self.root);
                    p
                };
                process.stdin(Stdio::null());
                if capture {
                    let out = process
                        .stderr(Stdio::inherit())
                        .output()
                        .with_context(|| format!("starting `{}`", cmd.program))?;
                    if !out.status.success() {
                        bail!("`{}` exited with {}", cmd.display(), out.status);
                    }
                    if let Some(sink) = sink {
                        sink(&String::from_utf8_lossy(&out.stdout))?;
                    }
                    Ok(())
                } else {
                    let status = process
                        .status()
                        .with_context(|| format!("starting `{}`", cmd.program))?;
                    if status.success() {
                        Ok(())
                    } else {
                        bail!("`{}` exited with {status}", cmd.display())
                    }
                }
            }
        }
    }
}

/// Runs the steps, prints the report and turns failures into an error.
///
/// # Errors
/// When a step failed.
pub fn run_with(runner: &mut Runner, steps: Vec<Step>) -> Result<()> {
    let report = runner.run(steps);
    print!("{}", report.render());
    report.into_result()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(name: &str, ok: bool) -> StepResult {
        StepResult {
            name: name.into(),
            place: "local".into(),
            status: if ok {
                Status::Ok
            } else {
                Status::Failed("boom".into())
            },
            secs: 1.25,
        }
    }

    #[test]
    fn exec_flags_select_the_mode() {
        assert_eq!(Exec::default().mode(), Mode::Auto);
        assert_eq!(
            Exec {
                local: true,
                remote: false
            }
            .mode(),
            Mode::Local
        );
        assert_eq!(
            Exec {
                local: false,
                remote: true
            }
            .mode(),
            Mode::Remote
        );
    }

    #[test]
    fn report_renders_a_table_and_the_failures() {
        let report = Report {
            results: vec![result("clippy", true), result("shear", false)],
        };
        let text = report.render();
        assert!(text.contains("clippy"));
        assert!(text.contains("FAILED"));
        assert!(text.contains("  shear: boom"));
        assert_eq!(
            report.into_result().unwrap_err().to_string(),
            "failed: shear"
        );
    }

    #[test]
    fn an_all_green_report_is_ok() {
        let report = Report {
            results: vec![result("a", true)],
        };
        assert!(report.into_result().is_ok());
    }

    #[test]
    fn local_runner_runs_calls_and_captures_commands() {
        let host = BuildHost::from_vars(None, None);
        let mut runner = Runner::with(host, Mode::Local, std::env::temp_dir());
        let seen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
        let sink = seen.clone();
        let steps = vec![
            Step::call("call", || Ok(())),
            Step::call("fail", || bail!("nope")),
            Step::build("rustc", Cmd::new("rustc").args(["--version"])).captured(move |out| {
                sink.borrow_mut().push_str(out);
                Ok(())
            }),
        ];
        let report = runner.run(steps);
        assert_eq!(report.results.len(), 3);
        assert_eq!(report.results[0].status, Status::Ok);
        assert_eq!(report.results[1].status, Status::Failed("nope".into()));
        assert_eq!(report.results[2].place, "local");
        assert!(seen.borrow().starts_with("rustc "));
    }
}
