---
name: binary-analysis
description: "Binary reverse engineering with Ghidra and yolo-re: triage an unknown binary, headless Ghidra analysis, live GhidrAssistMCP session, Ghidra GUI control, readable decompilation and renames, malware and firmware analysis, authorized CTF reversing, PyGhidra and local RAG. Use for any binary, firmware image or sample; REA runtime-evidence targets go to reverse-engineer-anything."
---
# Binary analysis: Ghidra, triage, malware, firmware, CTF

Start with `references/binary-triage.md` for an unknown binary, then take the reference that matches the next step.

Read the reference that matches the task in full before acting. Paths written inside reference files are relative to this skill's directory (the one holding this SKILL.md). Where a reference names another merged skill, open `references/<name>.md`; its helper files are under `references/<name>/`.

| Reference | Use when |
| --- | --- |
| [references/binary-triage.md](references/binary-triage.md) | Triage an unfamiliar binary with a fast static first pass covering architecture, imports, strings, capabilities, and concrete next steps. |
| [references/ghidra-workflows.md](references/ghidra-workflows.md) | Route common Ghidra tasks in Codex: installation status, binary triage, readable decompilation, vulnerability review, and headless batch analysis. |
| [references/ghidra-headless.md](references/ghidra-headless.md) | Runs official Ghidra headless analysis from yolo-re, exports decompilation evidence, and routes results through the Rust pipeline, Bun CLI, or REA MCP server. |
| [references/ghidra-mcp-live.md](references/ghidra-mcp-live.md) | Drive an interactive Ghidra GUI session through the live GhidrAssistMCP server (port 8080) to decompile, rename, list cross-references, define structures, and patch the program currently open in CodeBrowser. Use when the user has a binary loaded in Ghidra and wants real-time analysis on it, not batch processing of... |
| [references/ghidra-gui-computer-use.md](references/ghidra-gui-computer-use.md) | Operate the visible Ghidra CodeBrowser on Windows through Codex Computer Use for project navigation, display inspection, and user-authorized edits when the live Ghidra MCP server is unavailable or when the task is inherently visual. |
| [references/decompile-cleaner.md](references/decompile-cleaner.md) | Make Ghidra decompilation readable by recovering types and structures, identifying library calls, and proposing or applying precise symbol renames. |
| [references/malware-analyst.md](references/malware-analyst.md) | Perform static malware analysis to extract indicators, classify behavior, identify persistence and C2 patterns, and report evidence without executing samples. |
| [references/firmware-analyst.md](references/firmware-analyst.md) | Analyze embedded firmware, raw ARM or MIPS images, bootloaders, RTOS code, and peripheral access patterns with static Ghidra workflows. |
| [references/ctf-solver.md](references/ctf-solver.md) | Solve authorized reverse-engineering CTF challenges and compact puzzles using static analysis of common encodings, validation logic, and virtual machines. |

<!-- merge-skill-families:v1 -->
