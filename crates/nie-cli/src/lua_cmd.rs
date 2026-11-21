//! `nie lua`: static analysis of Lua sources, single file or tree.
//!
//! The analysis itself lives in [`nie_lua::static_analysis`], shared with the rest of the
//! engine. This module only walks the input and renders `key=value` lines.
//!
//! Every detail line repeats the file path, so the output stays greppable when the input is a
//! tree of many scripts, where a per-file header would be lost on the first redirect.

use std::path::Path;

use anyhow::{Context, bail};

/// Detail sections requested in addition to the summary line.
#[derive(Debug, Clone, Copy)]
pub struct Detail {
    /// Detail the declared functions.
    pub functions: bool,
    /// Detail the call targets, aggregated by frequency.
    pub calls: bool,
    /// Detail the retained string literals.
    pub strings: bool,
    /// Detail the strings passed to `CRC32`, with their hash.
    pub crc32: bool,
    /// Maximum lines per section and per file; `0` = unlimited.
    pub limit: usize,
}

impl Detail {
    /// Caps an item count by [`Self::limit`].
    fn take(self, total: usize) -> usize {
        if self.limit == 0 {
            total
        } else {
            total.min(self.limit)
        }
    }
}

/// Counters accumulated over a tree.
#[derive(Default)]
struct Totals {
    files: usize,
    valid: usize,
    functions: usize,
    calls: usize,
    tables: usize,
    strings: usize,
    crc32: usize,
    errors: usize,
}

/// Analyzes a `.lua` file or a tree and prints the result to standard output.
///
/// # Errors
/// If `src` does not exist, a single file is unreadable, or a tree contains no `.lua` file.
pub fn run(src: &Path, detail: Detail) -> anyhow::Result<()> {
    if src.is_dir() {
        let files = nie_lua::static_analysis::collect_lua_files(src);
        if files.is_empty() {
            bail!("no .lua file under {}", src.display());
        }
        let mut totals = Totals::default();
        for path in &files {
            // An unreadable file must not cancel the batch: report it and continue.
            match nie_lua::static_analysis::analyze_file(path) {
                Ok(a) => report(&a, path, detail, &mut totals),
                Err(e) => println!("lua={} ok=false reason={e}", path.display()),
            }
        }
        println!(
            "lua-total={} files={} valid={} functions={} calls={} tables={} strings={} \
             crc32={} errors={}",
            src.display(),
            totals.files,
            totals.valid,
            totals.functions,
            totals.calls,
            totals.tables,
            totals.strings,
            totals.crc32,
            totals.errors,
        );
        return Ok(());
    }

    let analysis = nie_lua::static_analysis::analyze_file(src)
        .with_context(|| format!("analyzing {}", src.display()))?;
    let mut totals = Totals::default();
    report(&analysis, src, detail, &mut totals);
    Ok(())
}

/// Prints a file's summary, then the requested detail sections.
fn report(
    a: &nie_lua::static_analysis::LuaAnalysis,
    path: &Path,
    detail: Detail,
    totals: &mut Totals,
) {
    let path = path.display();
    println!(
        "lua={path} ok={} lines={} functions={} calls={} assignments={} tables={} \
         strings={} crc32={} errors={}",
        a.is_valid(),
        a.line_count,
        a.functions.len(),
        a.calls.len(),
        a.assignments.len(),
        a.tables.len(),
        a.strings.len(),
        a.crc32_strings.len(),
        a.errors.len(),
    );

    totals.files += 1;
    totals.valid += usize::from(a.is_valid());
    totals.functions += a.functions.len();
    totals.calls += a.calls.len();
    totals.tables += a.tables.len();
    totals.strings += a.strings.len();
    totals.crc32 += a.crc32_strings.len();
    totals.errors += a.errors.len();

    // Errors are printed unasked: they are the only result that invalidates the rest of the
    // analysis, so hiding them by default would mislead.
    for e in a.errors.iter().take(detail.take(a.errors.len())) {
        println!(
            "error={path} line={} col={} kind={} text={}",
            e.line,
            e.column,
            e.kind.label(),
            e.text
        );
    }

    if detail.functions {
        for f in a.functions.iter().take(detail.take(a.functions.len())) {
            println!(
                "function={path} name={} kind={} params={} lines={}-{}",
                if f.name.is_empty() { "-" } else { &f.name },
                f.kind.label(),
                if f.params.is_empty() {
                    "-".to_string()
                } else {
                    f.params.join(",")
                },
                f.start_line,
                f.end_line,
            );
        }
    }

    if detail.calls {
        let counts = a.call_counts();
        for (name, n) in counts.iter().take(detail.take(counts.len())) {
            println!("call={path} name={name} count={n}");
        }
    }

    if detail.strings {
        for s in a.strings.iter().take(detail.take(a.strings.len())) {
            println!("string={path} value={s}");
        }
    }

    if detail.crc32 {
        for s in a
            .crc32_strings
            .iter()
            .take(detail.take(a.crc32_strings.len()))
        {
            // The engine addresses these names by hash: without it, the string does not
            // map to anything in the binary data.
            println!(
                "crc32={path} value={s} hash=0x{:08X}",
                nie_formats::cfgbin::crc32(s.as_bytes())
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_limite_zero_ne_borne_rien() {
        let d = Detail {
            functions: false,
            calls: false,
            strings: false,
            crc32: false,
            limit: 0,
        };
        assert_eq!(d.take(5000), 5000);
    }

    #[test]
    fn la_limite_borne_le_detail() {
        let d = Detail {
            functions: true,
            calls: false,
            strings: false,
            crc32: false,
            limit: 3,
        };
        assert_eq!(d.take(100), 3);
        // A section shorter than the limit is not extended.
        assert_eq!(d.take(2), 2);
    }

    #[test]
    fn un_repertoire_sans_lua_est_une_erreur() {
        let dir = std::env::temp_dir().join("nie-lua-vide");
        let _ = std::fs::create_dir_all(&dir);
        let d = Detail {
            functions: false,
            calls: false,
            strings: false,
            crc32: false,
            limit: 1,
        };
        assert!(run(&dir, d).is_err());
    }
}
