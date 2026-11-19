//! `apps/hello` is the output of `nie new hello --template cli` inside the workspace.

use std::path::Path;

use nie_scaffold::{Placement, Request, Template, generate};

#[test]
fn apps_hello_matches_its_template() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).expect("root");
    let tmp = tempfile::tempdir().expect("tmp");
    let dir = tmp.path().join("hello");
    let req = Request { name: "hello".into(), template: Template::Cli, dir: dir.clone(), placement: Placement::Workspace };
    let files = generate(&req).expect("generate");
    for rel in files {
        let want = std::fs::read_to_string(dir.join(&rel)).expect("generated");
        let have = std::fs::read_to_string(root.join("apps/hello").join(&rel)).unwrap_or_default();
        assert_eq!(have.replace("\r\n", "\n"), want, "apps/hello/{rel} drifted from the cli template");
    }
}
