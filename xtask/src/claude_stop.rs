//! `cargo xtask claude-stop`: Claude Code Stop hook that keeps the session working while the plan has
//! open tasks. Reads the hook JSON on stdin and the session's task store, blocks the stop with a minimal
//! JSON reason and gives up after `MAX` consecutive blocks with no task change.

use std::io::Read as _;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::{Value, json};

/// Consecutive blocks with an unchanged open-task signature before the hook lets the session stop.
const MAX: u64 = 25;

/// Arguments of `cargo xtask claude-stop` (the hook input arrives on stdin).
#[derive(clap::Args)]
pub struct Args {}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The open tasks of a task directory: (id, status, ready) in file name order.
fn open_tasks(dir: &Path) -> Vec<(String, String, bool)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|f| serde_json::from_str::<Value>(&std::fs::read_to_string(f).ok()?).ok())
        .filter_map(|t| {
            let status = t["status"].as_str()?.to_owned();
            if status == "completed" || status == "deleted" {
                return None;
            }
            let id = match &t["id"] {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let unblocked = t["blockedBy"].as_array().is_none_or(Vec::is_empty);
            let ready = status == "in_progress" || unblocked;
            Some((id, status, ready))
        })
        .collect()
}

/// The hook answer for the open tasks and the previous (signature, count) state, with the new state;
/// `None` lets the session stop.
fn decide(
    open: &[(String, String, bool)],
    previous: Option<(String, u64)>,
) -> (Option<Value>, (String, u64)) {
    let signature = open
        .iter()
        .map(|(id, status, _)| format!("{id}:{status}"))
        .collect::<Vec<_>>()
        .join(",");
    let count = match previous {
        Some((sig, n)) if sig == signature => n + 1,
        _ => 1,
    };
    let state = (signature, count);
    if open.is_empty() || count > MAX {
        return (None, state);
    }
    let next: Vec<&str> = open
        .iter()
        .filter(|t| t.2)
        .map(|t| t.0.as_str())
        .take(8)
        .collect();
    let reason = json!({ "goal": "complete the plan", "open": open.len(), "next": next, "rule": "act, verify, continue; no questions" });
    (
        Some(json!({ "decision": "block", "reason": reason.to_string() })),
        state,
    )
}

/// Reads the hook input, decides and prints the answer.
pub fn run(_args: Args) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let parsed: Value = serde_json::from_str(if input.trim().is_empty() {
        "{}"
    } else {
        &input
    })
    .unwrap_or(Value::Null);
    let sid: String = parsed["session_id"]
        .as_str()
        .unwrap_or_default()
        .chars()
        .take(8)
        .collect();
    let Some(home) = home() else { return Ok(()) };
    let dir = home
        .join(".claude")
        .join("tasks")
        .join(format!("session-{sid}"));
    if sid.is_empty() || !dir.is_dir() {
        return Ok(());
    }
    let open = open_tasks(&dir);
    if open.is_empty() {
        return Ok(());
    }
    let state_path = std::env::temp_dir().join(format!("nie-goal-{sid}.json"));
    let previous = std::fs::read_to_string(&state_path)
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| Some((v["sig"].as_str()?.to_owned(), v["n"].as_u64()?)));
    let (answer, (sig, n)) = decide(&open, previous);
    std::fs::write(&state_path, json!({ "sig": sig, "n": n }).to_string())?;
    if let Some(answer) = answer {
        println!("{answer}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, status: &str, ready: bool) -> (String, String, bool) {
        (id.to_owned(), status.to_owned(), ready)
    }

    #[test]
    fn open_tasks_block_with_the_ready_ids() {
        let open = [
            task("1", "in_progress", true),
            task("2", "pending", false),
            task("3", "pending", true),
        ];
        let (answer, state) = decide(&open, None);
        let answer = answer.unwrap();
        assert_eq!(answer["decision"], "block");
        let reason: Value = serde_json::from_str(answer["reason"].as_str().unwrap()).unwrap();
        assert_eq!(reason["next"], json!(["1", "3"]));
        assert_eq!(reason["open"], 3);
        assert_eq!(state, ("1:in_progress,2:pending,3:pending".to_owned(), 1));
    }

    #[test]
    fn the_hook_gives_up_after_max_unchanged_blocks() {
        let open = [task("1", "pending", true)];
        let (_, state) = decide(&open, None);
        let (answer, _) = decide(&open, Some((state.0.clone(), MAX)));
        assert!(answer.is_none());
        let (answer, _) = decide(&open, Some((state.0, MAX - 1)));
        assert!(answer.is_some());
        assert!(decide(&[], None).0.is_none());
    }

    #[test]
    fn a_changed_signature_restarts_the_count() {
        let (_, state) = decide(&[task("1", "pending", true)], Some(("old".into(), 20)));
        assert_eq!(state.1, 1);
    }

    #[test]
    fn the_task_store_is_read_from_json_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("1.json"),
            r#"{"id":"1","status":"completed"}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("2.json"),
            r#"{"id":"2","status":"pending","blockedBy":["1"]}"#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("3.json"),
            r#"{"id":"3","status":"in_progress","blockedBy":["2"]}"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("note.txt"), "x").unwrap();
        let open = open_tasks(dir.path());
        assert_eq!(
            open,
            [task("2", "pending", false), task("3", "in_progress", true)]
        );
    }
}
