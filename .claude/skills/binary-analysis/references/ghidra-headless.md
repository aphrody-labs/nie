<!-- Merged from skill `ghidra-headless` into `binary-analysis`. Original frontmatter:
name: ghidra-headless
description: >-
  Runs official Ghidra headless analysis from yolo-re, exports decompilation evidence,
  and routes results through the Rust pipeline, Bun CLI, or REA MCP server.
-->
# ghidra-headless

> Runs official Ghidra headless analysis from yolo-re, exports decompilation evidence, and routes results through the Rust pipeline, Bun CLI, or REA MCP server.


# Ghidra headless

Use the installed upstream Ghidra distribution through GHIDRA_INSTALL_DIR. Ghidra selects its
supported JDK from the install configuration; set JAVA_HOME when the runtime requires it.

## Pick an entry point

Run the native staged pipeline. Ghidra analysis and decompilation are optional external
stages; the report records per-stage status and partial exports:

~~~powershell
nie-re-core pipeline run .\sample.exe --target os --out .\out --external
~~~

Run the headless wrapper directly (`references/ghidra-headless/scripts/ghidra-analyze.ps1` or `.sh`;
`--help` lists its options).

## Rust and Java bridge

The Rust ghidra-headless adapter launches the official analyzeHeadless entry point,
writes a per-run Java GhidraScript, and accepts the result only when the UTF-8 function
inventory, decompiled output, per-function error records, summary, and completion marker
are consistent. On Windows it invokes the vendor batch launcher through cmd.exe while
keeping paths and arguments as separate environment values. The optional
ghidra-decompiler Cargo feature is a separate Rust implementation; it is not the Java
application or an official Ghidra FFI.

## Alpine Linux

The pinned, one-shot Linux headless profile is in infra/ghidra/alpine. It mounts the
input read-only, persists the project on the host, and runs without container network
access or Linux capabilities:

~~~powershell
./infra/ghidra/alpine/analyze.ps1 .\sample.exe .\ghidra-alpine-output
~~~

See its README for the Ghidra, JDK, and image pins and current Docker runtime validation
status.

## Output and failure handling

- The Bun wrapper stores launcher output and the Ghidra log in its selected output folder.
- Keep --process-timeout finite for unattended analyses. A timeout returns 124 and
  terminates the launcher process tree/group.
- Treat partial as incomplete evidence and inspect decompile-errors.jsonl; a
  completion marker alone does not imply every function decompiled.
- Use the generated report.json and artifact paths as the source of truth for pipeline
  status.
