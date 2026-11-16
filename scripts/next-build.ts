import { stat } from "node:fs/promises";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { checkBunRuntime } from "./bun-runtime";

export function assertWorkspaceMember(root: string, app: string): void {
  const member = relative(root, app);
  if (
    !member ||
    isAbsolute(member) ||
    member === ".." ||
    member.startsWith(`..${process.platform === "win32" ? "\\" : "/"}`)
  )
    throw new Error(
      "Next build must run from an app inside the Rose Griffon workspace",
    );
}

export async function assertFreshBuild(
  app: string,
  startedAt: number,
): Promise<void> {
  const path = join(app, ".next", "BUILD_ID");
  const artifact = await stat(path);
  if (!artifact.isFile() || artifact.mtimeMs < startedAt)
    throw new Error(`Next build output is missing or stale: ${path}`);
}

export async function buildNext(
  root: string,
  app: string,
  args: string[],
): Promise<number> {
  await checkBunRuntime(root);
  assertWorkspaceMember(root, app);
  const sync = Bun.spawn(
    [
      process.execPath,
      join(root, "packages/rg-assets/scripts/sync-website-public.ts"),
      "--to",
      join(app, "public"),
    ],
    { cwd: app, stdout: "inherit", stderr: "inherit" },
  );
  const syncExit = await sync.exited;
  if (syncExit !== 0) return syncExit;
  const startedAt = Date.now();
  const selectedBundler = args.some((arg) =>
    ["--webpack", "--turbopack", "--turbo"].includes(arg),
  )
    ? []
    : ["--webpack"];
  // Next sur Bun : `bun --bun next` (aucun Node requis sur la machine).
  const nextBin = join(
    dirname(Bun.resolveSync("next/package.json", app)),
    "dist",
    "bin",
    "next",
  );
  const next = Bun.spawn(
    [
      process.execPath,
      "--bun",
      nextBin,
      "build",
      ...selectedBundler,
      ...args,
    ],
    {
      cwd: app,
      env: { ...process.env, NEXT_TELEMETRY_DISABLED: process.env.NEXT_TELEMETRY_DISABLED ?? "1" },
      stdout: "inherit",
      stderr: "inherit",
      stdin: "inherit",
    },
  );
  const exitCode = await next.exited;
  if (exitCode !== 0) return exitCode;
  await assertFreshBuild(app, startedAt);
  return 0;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  if (args.includes("--analyze")) process.env.ANALYZE = "true";
  process.exitCode = await buildNext(
    resolve(import.meta.dir, ".."),
    process.cwd(),
    args.filter((arg) => arg !== "--analyze"),
  );
}
