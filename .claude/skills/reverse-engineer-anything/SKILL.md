---
name: reverse-engineer-anything
description: "Reverse engineer native, managed, Electron/JavaScript, packaged, firmware, and browser targets with REA. Use shipped-artifact or requested runtime evidence to explain features, compare versions, decompile code, or guide a reconstruction. Skip REA for ordinary source-repository architecture analysis. Also routes REA release changelogs, REA CI and test-suite audits, and REA tool, CLI and MCP contract design."
metadata:
  version: "42"
---
# REA: reverse engineer anything

`references/core.md` is the former body of this skill (REA investigation workflow): read it first. The REA project maintenance references follow.

Read the reference that matches the task in full before acting. Paths written inside reference files are relative to this skill's directory (the one holding this SKILL.md). Where a reference names another merged skill, open `references/<name>.md`; its helper files are under `references/<name>/`.

| Reference | Use when |
| --- | --- |
| [references/core.md](references/core.md) | Former body of this skill (read first) |
| [references/rea-changelog-update.md](references/rea-changelog-update.md) | Prepare or rewrite REA release changelogs and GitHub release notes from pinned Git history, with verified contributor thanks and Release Please synchronization. |
| [references/rea-ci-tests-audit.md](references/rea-ci-tests-audit.md) | Audit REA CI and test suites for correctness, fixture drift, false passes, and measured performance; use for suite or workflow investigations and efficiency changes, rather than ordinary feature testing or a single test failure. |
| [references/rea-tool-design.md](references/rea-tool-design.md) | Design or change REA investigation tools, CLI/MCP contracts, provider capabilities, and Evidence semantics. |

<!-- merge-skill-families:v1 -->
