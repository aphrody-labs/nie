// Headless Chrome proof of a page: navigator.gpu, console errors and a screenshot.
//
//   bun gui/tools/chrome-shot.ts <url> <out.png> [waitMs=8000] [width=1280] [height=760]
//
// Prints one JSON object: { gpu, adapter, errors, warnings, screenshot, title }. Exit code 1 when
// `navigator.gpu` is missing or a console error or exception was raised. Chrome comes from
// CHROME_BIN or `google-chrome`; WebGPU runs on SwiftShader (software Vulkan) so it works on a host
// without a GPU.
import { spawn } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const [url, out, waitArg, wArg, hArg] = process.argv.slice(2);
if (!url || !out) throw new Error("usage: chrome-shot.ts <url> <out.png> [waitMs] [width] [height]");
const waitMs = Number(waitArg ?? 8000);
const width = Number(wArg ?? 1280);
const height = Number(hArg ?? 760);
const port = 9300 + Math.floor(Math.random() * 500);
const profile = mkdtempSync(join(tmpdir(), "nie-chrome-"));

const chrome = spawn(
  process.env.CHROME_BIN ?? "google-chrome",
  [
    "--headless=new", "--no-sandbox", `--user-data-dir=${profile}`, `--remote-debugging-port=${port}`,
    `--window-size=${width},${height}`, "--enable-unsafe-webgpu", "--enable-features=Vulkan,WebGPU",
    "--use-angle=swiftshader", "--use-gl=angle", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist",
    "--disable-vulkan-surface", "about:blank",
  ],
  { stdio: "ignore" },
);

async function target(): Promise<string> {
  for (let i = 0; i < 100; i++) {
    try {
      const list = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()) as any[];
      const page = list.find((t) => t.type === "page");
      if (page) return page.webSocketDebuggerUrl;
    } catch {}
    await Bun.sleep(100);
  }
  throw new Error("Chrome did not start");
}

const ws = new WebSocket(await target());
await new Promise((r) => (ws.onopen = r));
let id = 0;
const pending = new Map<number, (v: any) => void>();
const errors: string[] = [];
const warnings: string[] = [];
ws.onmessage = (m) => {
  const msg = JSON.parse(String(m.data));
  if (msg.id && pending.has(msg.id)) return pending.get(msg.id)!(msg.result ?? msg.error);
  if (msg.method === "Runtime.consoleAPICalled") {
    const text = msg.params.args.map((a: any) => a.value ?? a.description ?? "").join(" ");
    if (msg.params.type === "error") errors.push(text);
    else if (msg.params.type === "warning") warnings.push(text);
  } else if (msg.method === "Runtime.exceptionThrown") {
    errors.push(msg.params.exceptionDetails.exception?.description ?? msg.params.exceptionDetails.text);
  } else if (msg.method === "Log.entryAdded" && msg.params.entry.level === "error") {
    errors.push(`${msg.params.entry.source}: ${msg.params.entry.text} ${msg.params.entry.url ?? ""}`);
  }
};
const send = (method: string, params: object = {}) =>
  new Promise<any>((resolve) => {
    const n = ++id;
    pending.set(n, resolve);
    ws.send(JSON.stringify({ id: n, method, params }));
  });

await send("Runtime.enable");
await send("Log.enable");
await send("Page.enable");
await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
await send("Page.navigate", { url });
await Bun.sleep(waitMs);
const probe = await send("Runtime.evaluate", {
  awaitPromise: true,
  returnByValue: true,
  expression: `(async () => {
    const gpu = typeof navigator.gpu !== "undefined";
    let adapter = null;
    if (gpu) { const a = await navigator.gpu.requestAdapter(); adapter = a ? (a.info?.description || a.info?.vendor || "adapter") : null; }
    return { gpu, adapter, title: document.title, canvases: document.querySelectorAll("canvas").length };
  })()`,
});
const shot = await send("Page.captureScreenshot", { format: "png" });
writeFileSync(out, Buffer.from(shot.data, "base64"));
const result = { ...probe.result.value, errors, warnings, screenshot: out };
console.log(JSON.stringify(result));
ws.close();
chrome.kill();
process.exit(result.gpu && errors.length === 0 ? 0 : 1);
