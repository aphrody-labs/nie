use super::*;

const SCML: &str = "open_flags = O_RDONLY | O_CLOEXEC;\nopenat(dirfd, path, flags = <open_flags>);\nclose(fd);\n";

fn fixture(log: &str) -> (tempfile::TempDir, std::path::PathBuf, Vec<std::path::PathBuf>) {
    let dir = tempfile::tempdir().unwrap();
    let scml = dir.path().join("fs.scml");
    std::fs::write(&scml, SCML).unwrap();
    let path = dir.path().join("strace.log");
    std::fs::write(&path, log).unwrap();
    (dir, path, vec![scml])
}

#[test]
fn unsupported_calls_are_aggregated_per_syscall() {
    let log = "100 openat(AT_FDCWD</>, \"/etc/hosts\", O_RDONLY|O_CLOEXEC) = 3</etc/hosts>\n\
               100 splice(3</etc/hosts>, NULL, 4<pipe:[1]>, NULL, 16, 0) = 16\n\
               100 splice(3</etc/hosts>, NULL, 4<pipe:[1]>, NULL, 16, 0) = 0\n\
               100 --- SIGCHLD {si_signo=SIGCHLD} ---\n\
               100 this line is not a syscall at all\n\
               100 close(3</etc/hosts>) = 0\n\
               100 +++ killed by SIGTERM +++\n";
    let (_dir, log, scml) = fixture(log);
    let (unsupported, parse_errors) = report::match_log(&log, &scml).unwrap();
    assert!(parse_errors <= 1);
    assert_eq!(unsupported.len(), 1);
    assert_eq!(unsupported[0].syscall, "splice");
    assert_eq!(unsupported[0].count, 2);
}

#[test]
fn strace_private_futex_names_match_the_spelled_out_flag() {
    let table = "futex(uaddr, futex_op = FUTEX_WAIT | FUTEX_PRIVATE_FLAG, val, timeout);\n";
    let dir = tempfile::tempdir().unwrap();
    let scml = dir.path().join("futex.scml");
    std::fs::write(&scml, table).unwrap();
    let log = dir.path().join("strace.log");
    std::fs::write(&log, "7 futex(0x7f00, FUTEX_WAIT_PRIVATE, 2, NULL) = 0\n").unwrap();
    let (unsupported, parse_errors) = report::match_log(&log, &[scml]).unwrap();
    assert!(unsupported.is_empty(), "{unsupported:?}");
    assert_eq!(parse_errors, 0);
}

#[test]
fn scml_files_are_found_recursively_and_sorted() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
    std::fs::write(dir.path().join("a/b/z.scml"), SCML).unwrap();
    std::fs::write(dir.path().join("a/y.scml"), SCML).unwrap();
    std::fs::write(dir.path().join("a/readme.md"), "").unwrap();
    let files = scml_files(dir.path()).unwrap();
    assert_eq!(files.len(), 2);
    assert!(files[0] < files[1]);
    assert!(scml_files(&dir.path().join("a/b")).is_ok());
    assert!(scml_files(&dir.path().join("a/readme.md")).is_err());
}

#[test]
fn a_traced_command_is_matched_end_to_end() {
    if std::process::Command::new("strace").arg("--version").output().is_err() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let scml = dir.path().join("none.scml");
    std::fs::write(&scml, SCML).unwrap();
    let policy = Policy::parse(
        "[[workload]]\nname = \"true\"\nargv = [\"/bin/sh\", \"-c\", \"exit 0\"]\n",
    )
    .unwrap();
    let ctx = Context { nie: "nie".into(), game: String::new() };
    let report = audit(dir.path(), dir.path(), &policy, &ctx).unwrap();
    assert_eq!(report.workloads.len(), 1);
    assert!(report.workloads[0].calls > 0);
    assert!(report.workloads[0].syscalls.contains_key("execve"));
    // The tiny table covers only openat and close, so everything else is a gap.
    assert!(report.unsupported.iter().any(|u| u.syscall == "execve"));
}
