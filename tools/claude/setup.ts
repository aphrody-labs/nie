// Applies `tools/claude/profile.toml` to Claude Code for this repository.
//   bun tools/claude/setup.ts          render .claude/settings.json and set the per-project MCP toggles
//   bun tools/claude/setup.ts --check  exit 1 when .claude/settings.json or the toggles drift
// Plugins, skills and MCP servers are discovered from the live install (settings.json,
// installed_plugins.json, known_marketplaces.json, plugin manifests, ~/.claude.json), so the profile
// only names what nie keeps.
//
// MCP scoping uses the per-project `disabledMcpServers` toggle (what `/mcp disable` writes in
// ~/.claude.json) because `allowedMcpServers` matches plugin stdio servers by their expanded,
// machine-specific command line. claude.ai connectors are dropped by `disableClaudeAiConnectors`.

import { existsSync, mkdirSync, readdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

type Profile = {
  mcp: { claude_ai_connectors: boolean; allow: string[] };
  plugins: Record<string, boolean>;
  skills: { on: string[]; name_only: string[] };
};
type Install = { scope: string; installPath: string; projectPath?: string };
type Manifest = { skills?: string | string[]; commands?: string | string[]; mcpServers?: string | Record<string, unknown> };

const root = join(import.meta.dir, "..", "..");
const projectKey = root.replace(/\\/g, "/");
const config = process.env.CLAUDE_CONFIG_DIR ?? join(homedir(), ".claude");
const stateFile = process.env.CLAUDE_CONFIG_DIR ? join(config, ".claude.json") : join(homedir(), ".claude.json");
const check = process.argv.includes("--check");
const profile = Bun.TOML.parse(readFileSync(join(import.meta.dir, "profile.toml"), "utf8")) as Profile;

const readJson = <T>(path: string): T | undefined =>
  existsSync(path) ? (JSON.parse(readFileSync(path, "utf8")) as T) : undefined;
const subdirs = (dir: string) =>
  existsSync(dir) ? readdirSync(dir, { withFileTypes: true }).filter((e) => e.isDirectory()).map((e) => e.name) : [];

function frontmatterName(file: string, fallback: string): string {
  const head = readFileSync(file, "utf8").split(/^---\s*$/m)[1] ?? "";
  return /^name:\s*["']?([^"'\r\n]+)/m.exec(head)?.[1]?.trim() ?? fallback;
}

function skillsIn(dir: string, prefix: string): string[] {
  const named = (name: string) => (prefix ? `${prefix}:${name}` : name);
  return subdirs(dir)
    .filter((name) => existsSync(join(dir, name, "SKILL.md")))
    .map((name) => named(frontmatterName(join(dir, name, "SKILL.md"), name)));
}

function commandsIn(dir: string, prefix: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir).filter((f) => f.endsWith(".md")).map((f) => `${prefix}:${f.slice(0, -3)}`);
}

// --- plugins -------------------------------------------------------------------------------
const userSettings = readJson<{ enabledPlugins?: Record<string, boolean> }>(join(config, "settings.json")) ?? {};
const enabled = { ...userSettings.enabledPlugins, ...profile.plugins };
const installed = readJson<{ plugins: Record<string, Install[]> }>(join(config, "plugins", "installed_plugins.json"))?.plugins ?? {};
const marketplaces =
  readJson<Record<string, { source: { source: string; path?: string }; installLocation: string }>>(
    join(config, "plugins", "known_marketplaces.json"),
  ) ?? {};

// Directory marketplaces load plugins in place from their source, not from the install cache.
function pluginRoot(id: string, install: Install): string {
  const [plugin, market] = id.split("@") as [string, string];
  const source = marketplaces[market]?.source;
  if (source?.source === "directory" && source.path) {
    const catalog = readJson<{ plugins: { name: string; source: string }[] }>(join(source.path, ".claude-plugin", "marketplace.json"));
    const entry = catalog?.plugins.find((p) => p.name === plugin);
    if (entry && typeof entry.source === "string") return join(source.path, entry.source);
  }
  return install.installPath;
}

const plugins: { name: string; dir: string; manifest: Manifest }[] = [];
for (const [id, installs] of Object.entries(installed)) {
  if (enabled[id] !== true) continue;
  const install = installs.find((e) => e.scope === "user" || e.projectPath === projectKey || e.projectPath === root);
  if (!install) continue;
  const dir = pluginRoot(id, install);
  const manifest = readJson<Manifest>(join(dir, ".claude-plugin", "plugin.json")) ?? readJson<Manifest>(join(dir, "plugin.json")) ?? {};
  plugins.push({ name: id.split("@")[0]!, dir, manifest });
}

// --- skills --------------------------------------------------------------------------------
const discovered = new Set(skillsIn(join(config, "skills"), ""));
for (const { name, dir, manifest } of plugins) {
  const skillDirs = manifest.skills === undefined ? ["skills"] : [manifest.skills].flat();
  const commandDirs = manifest.commands === undefined ? ["commands"] : [manifest.commands].flat();
  for (const d of skillDirs) for (const s of skillsIn(join(dir, d), name)) discovered.add(s);
  for (const d of commandDirs) for (const s of commandsIn(join(dir, d), name)) discovered.add(s);
}

const keep = new Set([...profile.skills.on, ...profile.skills.name_only]);
const missing = [...keep].filter((name) => !discovered.has(name));
if (missing.length) console.error(`profile keeps skills that are not installed: ${missing.join(", ")}`);

const skillOverrides: Record<string, string> = {};
for (const name of [...discovered].sort()) {
  if (profile.skills.name_only.includes(name)) skillOverrides[name] = "name-only";
  else if (!keep.has(name)) skillOverrides[name] = "off";
}

// --- MCP servers ---------------------------------------------------------------------------
const serverNames = (file: string | undefined): string[] => {
  const data = file ? readJson<Record<string, unknown>>(file) : undefined;
  if (!data) return [];
  return Object.keys((data.mcpServers as Record<string, unknown>) ?? data);
};
const state = readJson<{ mcpServers?: object; projects?: Record<string, Record<string, unknown>> }>(stateFile) ?? {};
const servers = new Map<string, string>(); // toggle name -> bare name used by the profile
for (const name of Object.keys(state.mcpServers ?? {})) servers.set(name, name);
for (const { name, dir, manifest } of plugins) {
  const declared = manifest.mcpServers;
  const names =
    typeof declared === "object" ? Object.keys(declared) : serverNames(typeof declared === "string" ? join(dir, declared) : join(dir, ".mcp.json"));
  for (const server of names) servers.set(`plugin:${name}:${server}`, server);
}
const disabled = [...servers].filter(([, bare]) => !profile.mcp.allow.includes(bare)).map(([toggle]) => toggle).sort();

// --- render --------------------------------------------------------------------------------
const mcpJson = readJson<{ mcpServers: object }>(join(root, ".mcp.json"))?.mcpServers ?? {};
const settings = {
  $schema: "https://json.schemastore.org/claude-code-settings.json",
  disableClaudeAiConnectors: !profile.mcp.claude_ai_connectors,
  enabledMcpjsonServers: Object.keys(mcpJson).filter((name) => profile.mcp.allow.includes(name)),
  enabledPlugins: Object.fromEntries(Object.entries(profile.plugins).sort(([a], [b]) => a.localeCompare(b))),
  skillOverrides,
};
const out = join(root, ".claude", "settings.json");
const text = `${JSON.stringify(settings, null, 2)}\n`;

const project = state.projects?.[projectKey] ?? {};
const current = [...((project.disabledMcpServers as string[]) ?? [])].sort();
const summary = `${discovered.size} skills (${Object.keys(skillOverrides).length} overridden), ${servers.size} MCP servers (${disabled.length} disabled)`;

if (check) {
  const drift: string[] = [];
  if ((existsSync(out) ? readFileSync(out, "utf8").replace(/\r\n/g, "\n") : "") !== text) drift.push(".claude/settings.json");
  if (JSON.stringify(current) !== JSON.stringify(disabled)) drift.push(`disabledMcpServers in ${stateFile}`);
  if (drift.length) {
    console.error(`drift: ${drift.join(", ")}; run bun tools/claude/setup.ts`);
    process.exit(1);
  }
  console.log(`up to date: ${summary}`);
} else {
  mkdirSync(join(root, ".claude"), { recursive: true });
  writeFileSync(out, text);
  // Re-read right before writing: running sessions rewrite ~/.claude.json too.
  const fresh = readJson<{ projects?: Record<string, Record<string, unknown>> }>(stateFile) ?? {};
  fresh.projects ??= {};
  fresh.projects[projectKey] = { ...fresh.projects[projectKey], disabledMcpServers: disabled };
  writeFileSync(`${stateFile}.nie-tmp`, JSON.stringify(fresh, null, 2));
  renameSync(`${stateFile}.nie-tmp`, stateFile);
  console.log(`applied: ${summary}\ndisabled: ${disabled.join(", ")}`);
}
