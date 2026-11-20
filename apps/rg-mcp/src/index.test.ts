// SPDX-License-Identifier: Apache-2.0
import { expect, test } from "bun:test";
import { resolve } from "node:path";

const entry = resolve(import.meta.dir, "index.ts");

/** Starts the stdio server, sends every message, and waits for one response per request id. */
async function exchange(
  messages: Array<Record<string, unknown>>,
  env: Record<string, string> = {},
) {
  const proc = Bun.spawn([process.execPath, entry], {
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, ...env },
  });
  const wanted = new Set(
    messages.filter((m) => m.id !== undefined).map((m) => m.id),
  );
  const responses = new Map<unknown, any>();
  const reader = proc.stdout.getReader();
  const decoder = new TextDecoder();
  let buffer = "";

  for (const m of messages) proc.stdin.write(`${JSON.stringify(m)}\n`);
  proc.stdin.flush();

  const deadline = Date.now() + 30_000;
  while (responses.size < wanted.size && Date.now() < deadline) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split("\n");
    buffer = lines.pop() ?? "";
    for (const line of lines) {
      if (!line.trim()) continue;
      const parsed = JSON.parse(line);
      responses.set(parsed.id, parsed);
    }
  }
  proc.stdin.end();
  await proc.exited;
  return responses;
}

const init = {
  jsonrpc: "2.0",
  id: 1,
  method: "initialize",
  params: {
    protocolVersion: "2024-11-05",
    capabilities: {},
    clientInfo: { name: "test-client", version: "1.0.0" },
  },
};

const call = (
  id: number,
  name: string,
  args: Record<string, unknown> = {},
) => ({
  jsonrpc: "2.0",
  id,
  method: "tools/call",
  params: { name, arguments: args },
});

test("stdio server initializes, lists the legacy tools first, and answers rg_status", async () => {
  const r = await exchange([
    init,
    { jsonrpc: "2.0", method: "notifications/initialized" },
    { jsonrpc: "2.0", id: 2, method: "tools/list", params: {} },
    call(3, "rg_status"),
  ]);
  expect(r.get(1).result.serverInfo.name).toBe("rosegriffon-mcp");

  const tools = r.get(2).result.tools as Array<{
    name: string;
    annotations: Record<string, boolean>;
  }>;
  expect(tools.slice(0, 11).map((t) => t.name)).toEqual([
    "rg_status",
    "rg_runtime_get",
    "rg_runtime_set",
    "rg_runtime_delete",
    "rg_runtime_audit",
    "rg_cron_inspect",
    "rg_redirects_list",
    "rg_crawl_404",
    "rg_vps_clean",
    "rg_docker_audit",
    "rg_vercel_audit",
  ]);
  expect(tools.length).toBeGreaterThan(100);
  const byName = new Map(tools.map((t) => [t.name, t.annotations]));
  expect(byName.get("rg_status")?.readOnlyHint).toBe(true);
  expect(byName.get("rg_runtime_delete")).toMatchObject({
    readOnlyHint: false,
    destructiveHint: true,
  });

  expect(r.get(3).result.content[0].text).toContain("operational");
});

test("stdio server executes rg_runtime_audit, rg_runtime_set dryRun, rg_cron_inspect and rg_vercel_audit", async () => {
  const r = await exchange([
    init,
    { jsonrpc: "2.0", method: "notifications/initialized" },
    call(10, "rg_runtime_audit"),
    call(11, "rg_runtime_set", {
      dotPath: "themes.esport",
      value: "achillea-light",
      dryRun: true,
    }),
    call(12, "rg_cron_inspect", { domain: "achillea" }),
    call(13, "rg_vercel_audit"),
  ]);
  const text = (id: number) => JSON.parse(r.get(id).result.content[0].text);

  expect(text(10).config.domains.achilleaUrl).toContain(
    "achillea.rosegriffon.fr",
  );
  expect(text(11).dryRun).toBe(true);
  expect(text(11).value.themes.esport).toBe("achillea-light");
  expect(text(12).count).toBeGreaterThan(0);
  expect(
    text(12).tasks.every((t: { domain: string }) => t.domain === "achillea"),
  ).toBe(true);
  expect(text(13).ok).toBe(true);
  expect(text(13).hasPrebuiltApiScript).toBe(true);
});

test("stdio tool errors come back as isError results, and RG_OPS_ROLE hides tools", async () => {
  const r = await exchange(
    [
      init,
      { jsonrpc: "2.0", method: "notifications/initialized" },
      call(20, "rg_cron_run", { task: "x" }),
    ],
    { CRON_SECRET: "" },
  );
  expect(r.get(20).result.isError).toBe(true);
  expect(r.get(20).result.content[0].text).toContain("CRON_SECRET");

  const readOnly = await exchange(
    [
      init,
      { jsonrpc: "2.0", method: "notifications/initialized" },
      { jsonrpc: "2.0", id: 21, method: "tools/list", params: {} },
    ],
    { RG_OPS_ROLE: "public" },
  );
  const names = (readOnly.get(21).result.tools as Array<{ name: string }>).map(
    (t) => t.name,
  );
  expect(names).toContain("rg_status");
  expect(names).not.toContain("rg_runtime_delete");
});

test("--http serves MCP, A2A and the agent card on RG_OPS_PORT", async () => {
  const port = 20_000 + Math.floor(Math.random() * 20_000);
  const proc = Bun.spawn([process.execPath, entry, "--http"], {
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...process.env,
      RG_OPS_PORT: String(port),
      RG_OPS_HOST: "127.0.0.1",
      RG_OPS_ADMIN_TOKEN: "t-admin",
    },
  });
  try {
    const base = `http://127.0.0.1:${port}`;
    let up = false;
    for (let i = 0; i < 100 && !up; i++) {
      up = await fetch(`${base}/healthz`).then(
        (r) => r.ok,
        () => false,
      );
      if (!up) await Bun.sleep(50);
    }
    expect(up).toBe(true);

    const card = (await (
      await fetch(`${base}/.well-known/agent-card.json`)
    ).json()) as { supportedInterfaces: Array<{ url: string }> };
    expect(card.supportedInterfaces[0]?.url).toBe(`${base}/a2a`);

    const mcp = await fetch(`${base}/mcp`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        accept: "application/json, text/event-stream",
        authorization: "Bearer t-admin",
      },
      body: JSON.stringify(call(1, "rg_status")),
    });
    const body = await mcp.text();
    const data = JSON.parse(
      body
        .split("\n")
        .find((l) => l.startsWith("data: "))!
        .slice(6),
    );
    expect(data.result.content[0].text).toContain("operational");

    const a2a = await fetch(`${base}/a2a`, {
      method: "POST",
      body: JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "SendMessage",
        params: { message: { messageId: "m", parts: [{ text: "status" }] } },
      }),
    });
    const task = ((await a2a.json()) as any).result.task;
    expect(task.status.state).toBe("TASK_STATE_COMPLETED");
    expect(task.artifacts[0].parts[0].data.status).toBe("operational");

    expect(
      (
        await fetch(`${base}/ops`, {
          headers: { authorization: "Bearer wrong" },
        })
      ).status,
    ).toBe(401);
  } finally {
    proc.kill();
    await proc.exited;
  }
});
