// Generates one SKILL.md per crate of the vendored Bevy, Dioxus, Blitz and Taffy forks (third_party/*)
// from the crate's own Cargo.toml, README, lib.rs and examples at the vendored commit.
// Usage (from the repo root): bun plugins/rosegriffon/scripts/gen-skills.ts
// Hand-written orientation text for the main crates lives in plugins/rosegriffon/hubs/<crate>.md.
import { readdirSync, readFileSync, statSync, writeFileSync, mkdirSync, existsSync, rmSync } from "fs";
import { join, relative } from "path";
import { spawnSync } from "child_process";

const ROOT = process.cwd();
const TP = join(ROOT, "third_party");
const OUT = join(ROOT, "plugins/rosegriffon/skills");
const REPOS: Record<string, string> = {
  bevy: "https://github.com/bevyengine/bevy",
  dioxus: "https://github.com/DioxusLabs/dioxus",
  blitz: "https://github.com/DioxusLabs/blitz",
  taffy: "https://github.com/DioxusLabs/taffy",
};
const WANT = /^(bevy|dioxus|blitz|taffy|manganis|subsecond|const-serialize|generational-box|wasm-split|stylo_taffy|accesskit_xplat|debug_timer|lazy-js-bundle|depinfo|dx-)/;
const SKIP_DIR = new Set([".git", "target", "node_modules", "tests", "benches", "docs", "scripts", "examples", "tools", "sites", "apps", "tests-integration", "fuzz", "assets", "src", "templates"]);
const posix = (p: string) => p.replaceAll("\\", "/");
const read = (p: string) => (existsSync(p) ? readFileSync(p, "utf8") : "");

type Crate = { name: string; repo: string; dir: string; toml: any; raw: string };
const crates: Crate[] = [];
const seen = new Set<string>();
function walk(repo: string, d: string) {
  for (const e of readdirSync(d)) {
    const p = join(d, e);
    if (e === "Cargo.toml" && d !== join(TP, repo)) {
      let t: any;
      try { t = Bun.TOML.parse(read(p)); } catch { continue; }
      const k = t.package;
      if (!k || !WANT.test(k.name) || k.publish === false) continue;
      if (repo === "blitz" && k.name.startsWith("dioxus-native")) continue; // the dioxus repo owns these names
      if (seen.has(k.name)) continue;
      seen.add(k.name);
      crates.push({ name: k.name, repo, dir: d, toml: t, raw: read(p) });
    } else if (statSync(p).isDirectory() && !SKIP_DIR.has(e)) walk(repo, p);
  }
}
for (const repo of Object.keys(REPOS)) {
  const root = join(TP, repo);
  const rt = read(join(root, "Cargo.toml"));
  if (/^\[package\]/m.test(rt)) { // the root crate (bevy, taffy)
    const t: any = Bun.TOML.parse(rt);
    if (WANT.test(t.package.name)) { seen.add(t.package.name); crates.push({ name: t.package.name, repo, dir: root, toml: t, raw: rt }); }
  }
  walk(repo, root);
}

const commit: Record<string, string> = {};
const ver: Record<string, string> = {};
for (const r of Object.keys(REPOS)) {
  commit[r] = spawnSync("git", ["-C", join(TP, r), "rev-parse", "HEAD"], { encoding: "utf8" }).stdout.trim();
  const wt: any = Bun.TOML.parse(read(join(TP, r, "Cargo.toml")));
  ver[r] = wt.workspace?.package?.version ?? wt.package?.version ?? "?";
}

function cleanReadme(s: string): string {
  return s
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/<[^>]+>/g, "")
    .split("\n")
    .filter((l) => !/^\s*\[!\[|^\s*!\[|^\s*\[[^\]]*\]:\s*http|^\[!\[/.test(l))
    .join("\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim()
    .split("\n").slice(0, 70).join("\n");
}
function innerDocs(lib: string): string {
  const m = lib.split("\n").filter((l) => l.startsWith("//!")).map((l) => l.replace(/^\/\/! ?/, ""));
  return m.slice(0, 60).join("\n").trim();
}
function features(raw: string): string[] {
  const out: string[] = [];
  const sec = raw.match(/^\[features\]\s*\n([\s\S]*?)(?=^\[|\Z)/m);
  if (!sec) return out;
  let doc: string[] = [];
  const lines = sec[1].split("\n");
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (l.startsWith("#")) { doc.push(l.replace(/^#+\s?/, "").trim()); continue; }
    const m = l.match(/^"?([A-Za-z0-9_\-]+)"?\s*=\s*(.*)$/);
    if (m) {
      let rhs = m[2];
      while (rhs.includes("[") && !rhs.includes("]") && i + 1 < lines.length) rhs += " " + lines[++i].trim();
      const members = (rhs.match(/"[^"]+"/g) ?? []).length;
      out.push(`- \`${m[1]}\`${members ? ` (enables ${members})` : ""}${doc.length ? `: ${doc.join(" ").slice(0, 200)}` : ""}`);
    }
    doc = [];
  }
  return out;
}
function rsFiles(d: string, acc: string[] = []): string[] {
  if (!existsSync(d)) return acc;
  for (const e of readdirSync(d)) {
    const p = join(d, e);
    if (statSync(p).isDirectory()) rsFiles(p, acc);
    else if (e.endsWith(".rs")) acc.push(p);
  }
  return acc;
}
function pubItems(c: Crate): string[] {
  const src = join(c.dir, "src");
  const files = rsFiles(src).sort().slice(0, 400);
  const byFile: string[] = [];
  let total = 0;
  for (const f of files) {
    const s = read(f);
    const names: string[] = [];
    const re = /^(?:pub(?:\([a-z]+\))?\s+(?:unsafe\s+|const\s+|async\s+)*(struct|enum|trait|fn|type|mod|union)\s+([A-Za-z_][A-Za-z0-9_]*)|pub\s+use\s+([^;]+);|#\[proc_macro_derive\(([A-Za-z_][A-Za-z0-9_]*)|#\[proc_macro_attribute\][\s\S]{0,40}?fn\s+([a-z_0-9]+)|#\[proc_macro\][\s\S]{0,40}?fn\s+([a-z_0-9]+))/gm;
    let m: RegExpExecArray | null;
    while ((m = re.exec(s))) {
      if (m[1] && !/^pub\s*$/.test(m[0]) && /pub\(/.test(m[0])) continue; // pub(crate) etc.
      if (m[1]) names.push(`${m[1]} ${m[2]}`);
      else if (m[4]) names.push(`derive ${m[4]}`);
      else if (m[5]) names.push(`attribute ${m[5]}`);
      else if (m[6]) names.push(`macro ${m[6]}`);
    }
    for (const m2 of s.matchAll(/#\[macro_export\][\s\S]{0,60}?macro_rules!\s+([A-Za-z_0-9]+)/g)) names.push(`macro ${m2[1]}!`);
    if (names.length) {
      total += names.length;
      byFile.push(`- \`${posix(relative(ROOT, f))}\`: ${[...new Set(names)].slice(0, 24).join(", ")}${names.length > 24 ? ", ..." : ""}`);
    }
  }
  return byFile.slice(0, 40).concat(byFile.length > 40 ? [`- (${byFile.length - 40} more files)`] : []);
}
let bevyExamples: string[] | null = null;
function examplesFor(c: Crate): string[] {
  const own = join(c.dir, "examples");
  const out: string[] = [];
  for (const f of rsFiles(own).slice(0, 8)) out.push(posix(relative(ROOT, f)));
  if (c.repo === "bevy") {
    bevyExamples ??= rsFiles(join(TP, "bevy/examples"));
    const id = c.name.replaceAll("-", "_") + "::";
    const key = c.name === "bevy" ? null : id;
    if (key) for (const f of bevyExamples) { if (out.length >= 8) break; if (read(f).includes(key)) out.push(posix(relative(ROOT, f))); }
  }
  return out;
}
function siblingDeps(c: Crate): string[] {
  const d = { ...(c.toml.dependencies ?? {}) } as Record<string, any>;
  return Object.keys(d).filter((k) => crates.some((x) => x.name === k) || WANT.test(k)).sort();
}
function oneLine(s: string) { return s.replace(/\s+/g, " ").trim().replace(/"/g, "'"); }

rmSync(OUT, { recursive: true, force: true });
mkdirSync(OUT, { recursive: true });
let n = 0;
for (const c of crates.sort((a, b) => a.name.localeCompare(b.name))) {
  const rel = posix(relative(ROOT, c.dir));
  const k = c.toml.package;
  const desc = k.description ?? "";
  const lib = read(join(c.dir, "src/lib.rs"));
  const readme = cleanReadme(read(join(c.dir, "README.md")));
  const docs = innerDocs(lib);
  const hub = read(join(ROOT, "plugins/rosegriffon/hubs", `${c.name}.md`));
  const feats = features(c.raw);
  const items = pubItems(c);
  const ex = examplesFor(c);
  const deps = siblingDeps(c);
  const version = typeof k.version === "string" ? k.version : ver[c.repo];
  const title = `Use when working with the \`${c.name}\` crate of ${c.repo}${desc ? `: ${desc.replace(/\.$/, "")}` : ""}.`;
  const body = [
    "---",
    `name: ${c.name.replaceAll("_", "-")}`,
    `description: "${oneLine(title).slice(0, 300)}"`,
    "---",
    "",
    `# ${c.name}`,
    "",
    `- Version: ${version} (workspace member of \`third_party/${c.repo}\`, vendored commit \`${commit[c.repo].slice(0, 12)}\`)`,
    `- Source: \`${rel}/\` (manifest \`${rel}/Cargo.toml\`)`,
    `- Upstream: ${REPOS[c.repo]}/tree/main/${rel.replace(`third_party/${c.repo}/`, "").replace(`third_party/${c.repo}`, "")}`,
    `- License: ${k.license ?? "see manifest"}`,
    "- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.",
    "",
    ...(hub ? ["## Orientation", "", hub.trim(), ""] : []),
    ...(desc ? ["## Purpose", "", desc, ""] : []),
    ...(readme ? [`## README (\`${rel}/README.md\`)`, "", readme, ""] : docs ? [`## Crate docs (\`${rel}/src/lib.rs\`)`, "", docs, ""] : []),
    ...(feats.length ? ["## Cargo features (from the manifest)", "", ...feats.slice(0, 60), ""] : []),
    ...(deps.length ? ["## Depends on (same ecosystem)", "", deps.map((d) => `\`${d}\``).join(", "), ""] : []),
    ...(items.length ? ["## Public items by file", "", ...items, ""] : []),
    ...(ex.length ? ["## Examples in the source tree", "", ...ex.map((e) => `- \`${e}\``), ""] : []),
    "## How to use this skill",
    "",
    "Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.",
    "",
  ].join("\n");
  mkdirSync(join(OUT, c.name.replaceAll("_", "-")), { recursive: true });
  writeFileSync(join(OUT, c.name.replaceAll("_", "-"), "SKILL.md"), body);
  n++;
}
console.log(`${n} skills in ${posix(relative(ROOT, OUT))}`, commit);

// Index of the monorepo skills: .agents/skills stays the single source (AGENTS.md); the plugin only points at it.
{
  const dir = join(ROOT, ".agents/skills");
  const rows: string[] = [];
  for (const e of readdirSync(dir).sort()) {
    const f = join(dir, e, "SKILL.md");
    if (!existsSync(f)) continue;
    const fm = read(f).match(/^---\n([\s\S]*?)\n---/)?.[1] ?? "";
    const d = fm.match(/^description:\s*(.*)$/m)?.[1]?.replace(/^["']|["']$/g, "") ?? "";
    rows.push(`- \`.agents/skills/${e}/SKILL.md\`: ${d.slice(0, 220)}`);
  }
  const body = [
    "---",
    "name: rosegriffon-monorepo-skills",
    'description: "Use when a task touches the Rose Griffon monorepo domains (auth, Next.js, Inazuma Eleven formats, nie studio, reverse engineering, Steam, mods): lists the project skills kept in .agents/skills, the single source, with the path of each."',
    "---",
    "",
    "# Monorepo skills (single source: `.agents/skills/`)",
    "",
    "`AGENTS.md` makes `.agents/skills/` the one source of project skills, read by every agent CLI. This plugin does not copy them: open the `SKILL.md` below that matches the task and follow it.",
    "",
    ...rows,
    "",
    "Ecosystem skills for Bevy, Dioxus, Blitz, Taffy, manganis and subsecond are generated into this plugin by `plugins/rosegriffon/scripts/gen-skills.ts` from the vendored forks.",
    "",
  ].join("\n");
  mkdirSync(join(OUT, "rosegriffon-monorepo-skills"), { recursive: true });
  writeFileSync(join(OUT, "rosegriffon-monorepo-skills", "SKILL.md"), body);
}
