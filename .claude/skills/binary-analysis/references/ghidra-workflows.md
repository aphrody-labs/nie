<!-- Merged from skill `ghidra-workflows` into `binary-analysis`. Original frontmatter:
name: ghidra-workflows
description: "Route common Ghidra tasks in Codex: installation status, binary triage, readable decompilation, vulnerability review, and headless batch analysis."
-->
# ghidra-workflows

> Route common Ghidra tasks in Codex: installation status, binary triage, readable decompilation, vulnerability review, and headless batch analysis.


# Ghidra workflows

Choose the smallest appropriate route:

- **Status and setup:** read [references/ghidra-workflows/status.md](references/ghidra-workflows/status.md).
- **First-pass binary orientation:** read [references/ghidra-workflows/triage.md](references/ghidra-workflows/triage.md) and use `binary-triage`.
- **Readability cleanup for an open program:** read [references/ghidra-workflows/decompile.md](references/ghidra-workflows/decompile.md) and use `decompile-cleaner` plus `ghidra-mcp-live`.
- **Authorized static vulnerability assessment:** read [references/ghidra-workflows/vulns.md](references/ghidra-workflows/vulns.md) and use `vulnerability-hunter`.
- **Directory-scale static exports:** read [references/ghidra-workflows/batch.md](references/ghidra-workflows/batch.md) and use `ghidra-headless`.

All routes are static analysis only. Confirm exact output paths before headless batch jobs, and use the live MCP whenever a loaded Ghidra program needs structured access.
