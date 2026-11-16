---
name: rosegriffon-monorepo-skills
description: "Use when a task touches the Rose Griffon monorepo domains (auth, Next.js, Inazuma Eleven formats, nie studio, reverse engineering, Steam, mods): lists the project skills kept in .agents/skills, the single source, with the path of each."
---

# Monorepo skills (single source: `.agents/skills/`)

`AGENTS.md` makes `.agents/skills/` the one source of project skills, read by every agent CLI. This plugin does not copy them: open the `SKILL.md` below that matches the task and follow it.

- `.agents/skills/3ds-reverse/SKILL.md`: Analyze Nintendo 3DS CIA, CCI, CXI, and 3DSX software, including ARM11, Horizon services, ExeFS, and RomFS with Ghidra and Azahar artifacts.
- `.agents/skills/ai-sdk/SKILL.md`: Vercel AI SDK expert guidance. Use when building AI-powered features — chat interfaces, text generation, structured output, tool calling, agents, MCP integration, streaming, embeddings, reranking, image generation, or wo
- `.agents/skills/auth/SKILL.md`: Authentication integration guidance — Clerk (native Vercel Marketplace), Better Auth, Descope, and Auth0 setup for Next.js applications, plus Sign in with Vercel, Vercel Passport, and Vercel KMS. Covers server config, ro
- `.agents/skills/base-ui/SKILL.md`: Official Base UI (@base-ui/react v1.9+) reference and composition patterns for building accessible, unstyled React primitives. Use when working with @base-ui/react components, render props, data attributes, forms, or Typ
- `.agents/skills/better-auth-best-practices/SKILL.md`: Configure Better Auth server and client, set up database adapters, manage sessions, add plugins, and handle environment variables. Use when users mention Better Auth, betterauth, auth.ts, or need to set up TypeScript aut
- `.agents/skills/better-auth-security-best-practices/SKILL.md`: Configure rate limiting, manage auth secrets, set up CSRF protection, define trusted origins, secure sessions and cookies, encrypt OAuth tokens, track IP addresses, and implement audit logging for Better Auth. Use when u
- `.agents/skills/binary-triage/SKILL.md`: Triage an unfamiliar binary with a fast static first pass covering architecture, imports, strings, capabilities, and concrete next steps.
- `.agents/skills/chat-sdk/SKILL.md`: Vercel Chat SDK expert guidance. Use when building multi-platform chat bots — Slack, Telegram, Microsoft Teams, Discord, Google Chat, GitHub, Linear — with a single codebase. Covers the Chat class, adapters, threads, mes
- `.agents/skills/create-auth/SKILL.md`: Scaffold and implement authentication in TypeScript/JavaScript apps using Better Auth. Detect frameworks, configure database adapters, set up route handlers, add OAuth providers, and create auth UI pages. Use when users 
- `.agents/skills/ctf-solver/SKILL.md`: Solve authorized reverse-engineering CTF challenges and compact puzzles using static analysis of common encodings, validation logic, and virtual machines.
- `.agents/skills/decompile-cleaner/SKILL.md`: Make Ghidra decompilation readable by recovering types and structures, identifying library calls, and proposing or applying precise symbol renames.
- `.agents/skills/ds-reverse/SKILL.md`: Analyze Nintendo DS and DSi ROMs with NTRGhidra, including ARM9, ARM7, overlays, NitroSDK functions, and hardware register maps.
- `.agents/skills/email-and-password-best-practices/SKILL.md`: Configure email verification, implement password reset flows, set password policies, and customise hashing algorithms for Better Auth email/password authentication. Use when users need to set up login, sign-in, sign-up, 
- `.agents/skills/firmware-analyst/SKILL.md`: Analyze embedded firmware, raw ARM or MIPS images, bootloaders, RTOS code, and peripheral access patterns with static Ghidra workflows.
- `.agents/skills/gamebanana/SKILL.md`: Search and inspect public GameBanana submissions through the read-only GameBanana API, with links to original mod pages.
- `.agents/skills/ghidra-gui-computer-use/SKILL.md`: Operate the visible Ghidra CodeBrowser on Windows through Codex Computer Use for project navigation, display inspection, and user-authorized edits when the live Ghidra MCP server is unavailable or when the task is inhere
- `.agents/skills/ghidra-headless/SKILL.md`: >-
- `.agents/skills/ghidra-mcp-live/SKILL.md`: Drive an interactive Ghidra GUI session through the live GhidrAssistMCP server (port 8080) to decompile, rename, list cross-references, define structures, and patch the program currently open in CodeBrowser. Use when the
- `.agents/skills/ghidra-workflows/SKILL.md`: Route common Ghidra tasks in Codex: installation status, binary triage, readable decompilation, vulnerability review, and headless batch analysis.
- `.agents/skills/ie-game-formats/SKILL.md`: Inspect and extract Inazuma Eleven / Level-5 / Criware assets with the native Rust decoders exposed through REA and MCP.
- `.agents/skills/malware-analyst/SKILL.md`: Perform static malware analysis to extract indicators, classify behavior, identify persistence and C2 patterns, and report evidence without executing samples.
- `.agents/skills/migrate-radix-to-base/SKILL.md`: Migrates React projects and components from Radix UI to Base UI. Use when asked to migrate from radix, move to base-ui, convert radix primitives, or switch a shadcn project's base library. Handles single components ("mig
- `.agents/skills/mod-package/SKILL.md`: Validate and inspect mod package manifests and use common package safety conventions across GameBanana, Modrinth, Thunderstore and mod managers.
- `.agents/skills/next-bundle-optimizer/SKILL.md`: >
- `.agents/skills/next-cache-components-adoption/SKILL.md`: >
- `.agents/skills/next-cache-components-optimizer/SKILL.md`: >
- `.agents/skills/next-dev-loop/SKILL.md`: >
- `.agents/skills/next-partial-prefetching-adoption/SKILL.md`: >
- `.agents/skills/next-partial-prefetching-optimizer/SKILL.md`: >
- `.agents/skills/nie-studio/SKILL.md`: Unified studio suite for Inazuma Eleven Victory Road (NIE) covering Save Editor, Mod Manager, CPK Explorer, 3D Character Morphing, FilmCraft/Remotion Cinematic Timeline, and Multimodal RAG Dataset Ingestion.
- `.agents/skills/organization-best-practices/SKILL.md`: Configure multi-tenant organizations, manage members and invitations, define custom roles and permissions, set up teams, and implement RBAC using Better Auth's organization plugin. Use when users need org setup, team man
- `.agents/skills/python-re/SKILL.md`: Use yolo-re's isolated uv Python project for binary RE, target evidence ingestion, local RAG/ML, and native PyGhidra.
- `.agents/skills/react-best-practices/SKILL.md`: Vercel's React and Next.js performance rules (70 rules in 8 categories ordered by impact - eliminating waterfalls, bundle size, server-side performance, client-side data fetching, re-renders, rendering performance, JavaS
- `.agents/skills/shadcn/SKILL.md`: Manages shadcn components and projects — adding, searching, fixing, debugging, styling, and composing UI, including chat interfaces. Provides project context, component docs, and usage examples. Applies when working with
- `.agents/skills/static-recompile/SKILL.md`: Statically recompile console BIOS and ROM images (Game Boy Advance ARMv4T ARM/Thumb, PlayStation MIPS R3000A BIOS and PS-EXE, NES 6502) to native executables and WebAssembly with `re recompile`, and prove the result agai
- `.agents/skills/steam/SKILL.md`: Steam on Bun OS through bun-steam - client config, steamcmd with the cached session, libraries, Proton (GE, CachyOS, Valve, official anti-cheat runtimes) and launch tuning, Remote Play, VDF, depots, and the local Steam i
- `.agents/skills/switch-reverse/SKILL.md`: Use this skill when investigating Nintendo Switch binaries (NSP/NCA/NSO) — extracting code via hactool, parsing NSO format (sections, dynsym, dynstr, JMPREL), identifying which symbol a PLT call address resolves to, comp
- `.agents/skills/two-factor-authentication-best-practices/SKILL.md`: Configure TOTP authenticator apps, send OTP codes via email/SMS, manage backup codes, handle trusted devices, and implement 2FA sign-in flows using Better Auth's twoFactor plugin. Use when users need MFA, multi-factor au
- `.agents/skills/uv-project/SKILL.md`: Maintain yolo-re's isolated Python RE/ML project under src/py with uv.
- `.agents/skills/vercel-react-best-practices/SKILL.md`: React and Next.js performance optimization guidelines from Vercel Engineering. This skill should be used when writing, reviewing, or refactoring React/Next.js code to ensure optimal performance patterns. Triggers on task

Ecosystem skills for Bevy, Dioxus, Blitz, Taffy, manganis and subsecond are generated into this plugin by `plugins/rosegriffon/scripts/gen-skills.ts` from the vendored forks.
