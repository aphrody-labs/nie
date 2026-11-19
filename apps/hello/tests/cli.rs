use std::process::Command;

#[test]
fn library_and_binary_agree() {
    let (code, out) = hello::run(&[]);
    assert_eq!(code, 0);
    let bin = Command::new(env!("CARGO_BIN_EXE_hello")).output().expect("run");
    assert!(bin.status.success());
    assert_eq!(String::from_utf8_lossy(&bin.stdout), out);
}

#[test]
fn unknown_command_fails() {
    assert_eq!(hello::run(&["nope".to_owned()]).0, 2);
}

#[test]
fn plugin_declares_its_command() {
    use nie_plugin::Plugin as _;
    assert_eq!(hello::HelloPlugin.info().commands, &["hello"]);
}
