//! Text previews: plain text, Lua source, Lua bytecode and event scripts.

use std::fmt::Write as _;

use nie_formats::cfgbin::Value;
use nie_formats::event_script;
use nie_lua::bytecode::{self, Chunk, Constant, Prototype, decode_instruction};

use super::{Body, Built, Line, Style, hex};
use crate::fm::highlight;

/// Lines kept for a text view.
const MAX_LINES: usize = 4000;

/// Whether the head of a file looks like text: UTF-8 with no control character but `\n \r \t`.
pub(super) fn is_text(head: &[u8]) -> bool {
    if head.is_empty() {
        return false;
    }
    let head = &head[..head.len().min(512)];
    let valid = match std::str::from_utf8(head) {
        Ok(_) => true,
        // A multi-byte character cut by the window is still text.
        Err(e) => e.error_len().is_none() && e.valid_up_to() + 4 > head.len(),
    };
    valid
        && head
            .iter()
            .all(|b| *b >= 0x20 || matches!(b, b'\n' | b'\r' | b'\t'))
}

fn capped(mut lines: Vec<Line>, meta: &mut Vec<(String, String)>) -> Vec<Line> {
    meta.push(("lines".to_owned(), lines.len().to_string()));
    if lines.len() > MAX_LINES {
        lines.truncate(MAX_LINES);
        lines.push(Line::new(format!("... truncated at {MAX_LINES} lines"), Style::Dim));
    }
    lines
}

pub(super) fn plain(data: &[u8]) -> Built {
    let text = String::from_utf8_lossy(data);
    let mut meta = vec![("encoding".to_owned(), "UTF-8".to_owned())];
    let lines = text
        .lines()
        .take(MAX_LINES + 1)
        .map(|l| Line::new(l.to_owned(), Style::Plain))
        .collect();
    let lines = capped(lines, &mut meta);
    (Body::Text(lines), meta, false)
}

pub(super) fn lua_source(data: &[u8]) -> Built {
    let text = String::from_utf8_lossy(data);
    let mut meta = vec![("language".to_owned(), "Lua".to_owned())];
    let lines = capped(highlight::lua(&text), &mut meta);
    (Body::Text(lines), meta, false)
}

/// `GETTABUP(0, 0, 3)  -- _ENV["print"]`: one instruction per call, valid Lua so the grammar can
/// colour it.
fn bytecode_source(chunk: &Chunk) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- Lua {:#04x} bytecode, {} prototype(s), {} instruction(s)",
        chunk.header.version,
        chunk.main.total_protos() + 1,
        chunk.main.total_instructions()
    );
    proto(&chunk.main, "main", 0, &mut out);
    out
}

fn constant_text(p: &Prototype, index: u32) -> Option<String> {
    p.constants.get(index as usize).map(|c| match c {
        Constant::String(bytes) => format!("{:?}", String::from_utf8_lossy(bytes)),
        other => other.display(),
    })
}

/// An RK operand: a constant when the high bit is set, a register otherwise.
fn rk(p: &Prototype, operand: u32) -> Option<String> {
    (operand >= 256).then(|| constant_text(p, operand - 256)).flatten()
}

fn proto(p: &Prototype, label: &str, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    let name = label.replace(':', "_");
    let _ = writeln!(
        out,
        "{pad}local function {name}() -- {} param(s), {} upvalue(s), lines {}-{}",
        p.num_params,
        p.upvalues.len(),
        p.line_defined,
        p.last_line_defined
    );
    for (pc, raw) in p.code.iter().enumerate() {
        let ins = decode_instruction(*raw);
        let op = ins.name();
        let args = match ins.mode() {
            bytecode::OpMode::ABC => format!("{}, {}, {}", ins.a, ins.b, ins.c),
            bytecode::OpMode::ABx => format!("{}, {}", ins.a, ins.bx),
            bytecode::OpMode::AsBx => format!("{}, {}", ins.a, ins.sbx),
            bytecode::OpMode::Ax => format!("{}", ins.ax),
        };
        let note = match op.as_str() {
            "LOADK" => constant_text(p, ins.bx),
            "GETTABUP" | "GETTABLE" | "SELF" => rk(p, ins.c),
            "SETTABUP" | "SETTABLE" => {
                let (b, c) = (rk(p, ins.b), rk(p, ins.c));
                match (b, c) {
                    (Some(b), Some(c)) => Some(format!("{b} = {c}")),
                    (Some(b), None) => Some(b),
                    (None, c) => c,
                }
            }
            "EQ" | "LT" | "LE" | "ADD" | "SUB" | "MUL" | "DIV" | "MOD" | "POW" => {
                match (rk(p, ins.b), rk(p, ins.c)) {
                    (Some(b), Some(c)) => Some(format!("{b}, {c}")),
                    (Some(x), None) | (None, Some(x)) => Some(x),
                    (None, None) => None,
                }
            }
            "CLOSURE" => Some(format!("{name}_{}", ins.bx)),
            _ => None,
        };
        let line = p.line_info.get(pc).map(|l| format!("line {l}"));
        let comment = match (note, line) {
            (Some(n), Some(l)) => format!(" -- {pc}: {n} ({l})"),
            (Some(n), None) => format!(" -- {pc}: {n}"),
            (None, Some(l)) => format!(" -- {pc} ({l})"),
            (None, None) => format!(" -- {pc}"),
        };
        let _ = writeln!(out, "{pad}  {op}({args}){comment}");
    }
    for (i, sub) in p.protos.iter().enumerate() {
        proto(sub, &format!("{label}_{i}"), depth + 1, out);
    }
    let _ = writeln!(out, "{pad}end");
}

pub(super) fn lua_bytecode(data: &[u8]) -> Built {
    match bytecode::parse(data) {
        Ok(chunk) => {
            let mut meta = vec![
                ("format".to_owned(), "Lua 5.2 bytecode".to_owned()),
                ("view".to_owned(), "disassembly in call syntax".to_owned()),
            ];
            let lines = capped(highlight::lua(&bytecode_source(&chunk)), &mut meta);
            (Body::Text(lines), meta, false)
        }
        Err(error) => hex::message_with_hex(data, &format!("lua bytecode: {error}")),
    }
}

fn argument(v: &Value) -> String {
    match v {
        Value::String(s) => format!("{s:?}"),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 {
                format!("{f:.1}")
            } else {
                format!("{f}")
            }
        }
    }
}

/// `cmd(120, "Animation", 0xD552FA5D, "c01_walk", 3)`: one command per call.
fn event_source(doc: &event_script::EventScriptDocument) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "-- event script: {} command(s), {} cut(s), {} frame(s)",
        doc.len(),
        doc.cuts().len(),
        doc.duration_frames()
    );
    for command in &doc.commands {
        let args: String = command
            .args
            .iter()
            .map(|a| format!(", {}", argument(a)))
            .collect();
        let _ = writeln!(
            out,
            "cmd({}, \"{:?}\", 0x{:08X}{args})",
            command.frame, command.category, command.opcode
        );
    }
    out
}

pub(super) fn event(data: &[u8]) -> Built {
    match event_script::parse(data) {
        Ok(doc) => {
            let mut meta = vec![
                ("format".to_owned(), "event script (T2B)".to_owned()),
                ("view".to_owned(), "commands in call syntax".to_owned()),
            ];
            let lines = capped(highlight::lua(&event_source(&doc)), &mut meta);
            (Body::Text(lines), meta, false)
        }
        Err(error) => hex::message_with_hex(data, &format!("event script: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_detection() {
        assert!(is_text(b"hello\nworld\t!"));
        assert!(is_text("caf\u{e9}".as_bytes()));
        assert!(!is_text(&[0, 1, 2]));
        assert!(!is_text(b""));
        assert!(!is_text(b"G4TX\x00\x00"));
    }

    #[test]
    fn plain_text_keeps_its_lines() {
        let (body, meta, _) = plain(b"a\nb\n");
        let Body::Text(lines) = body else { panic!("text") };
        assert_eq!(lines.len(), 2);
        assert!(meta.iter().any(|(k, v)| k == "lines" && v == "2"));
    }

    #[test]
    fn event_commands_render_as_highlightable_lua() {
        let doc = event_script::EventScriptDocument {
            header_command_count: 1,
            commands: vec![event_script::EventCommand {
                index: 0,
                frame: 12,
                opcode: 0xFD04_4302,
                category: event_script::EventCategory::Cut,
                args: vec![Value::String("cut_01".into()), Value::Int(3), Value::Float(0.5)],
            }],
        };
        let source = event_source(&doc);
        assert!(source.contains("cmd(12, \"Cut\", 0xFD044302, \"cut_01\", 3, 0.5)"));
        let lines = highlight::lua(&source);
        assert!(lines
            .iter()
            .flat_map(|l| &l.0)
            .any(|s| s.text.contains("cut_01") && s.style == Style::String));
    }
}
