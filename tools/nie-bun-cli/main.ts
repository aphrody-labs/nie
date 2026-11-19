import { CString, dlopen, FFIType, ptr, suffix } from "bun:ffi";
import { existsSync } from "node:fs";
import { resolve } from "node:path";

const DEFAULT_HTTP_URL = process.env.NIE_HTTP_URL ?? "http://127.0.0.1:8790";

function candidateFfiPaths(): string[] {
  const libName = process.platform === "win32" ? `nie.${suffix}` : `libnie.${suffix}`;
  const list: string[] = [];
  if (process.env.NIE_FFI_LIB) list.push(process.env.NIE_FFI_LIB);
  list.push(
    resolve(process.cwd(), "target", "release", libName),
    resolve(process.cwd(), "target", "debug", libName),
    resolve(process.cwd(), libName),
    resolve(import.meta.dir, "..", "..", "target", "release", libName),
    resolve(import.meta.dir, "..", "..", "target", "debug", libName),
  );
  return [...new Set(list)];
}

function openNieAbi() {
  const searchedPaths = candidateFfiPaths();
  const foundPath = searchedPaths.find((p) => existsSync(p)) ?? null;
  if (!foundPath) {
    return {
      status: {
        loaded: false,
        path: null,
        version: null,
        crc32SelfTest: null,
        searchedPaths,
        error: "nie-ffi shared library not found",
      },
      lib: null,
    };
  }
  try {
    const lib = dlopen(foundPath, {
      nie_version: { args: [], returns: FFIType.ptr },
      nie_crc32: { args: [FFIType.ptr, FFIType.u64], returns: FFIType.u32 },
      nie_detect: { args: [FFIType.ptr, FFIType.u64], returns: FFIType.u32 },
      nie_format_name: { args: [FFIType.u32], returns: FFIType.ptr },
      nie_bytes_free_fields: { args: [FFIType.u64, FFIType.u64, FFIType.u64], returns: FFIType.void },
    });
    const verPtr = lib.symbols.nie_version();
    const version = verPtr ? new CString(verPtr).toString() : null;
    const probe = Buffer.from("123456789", "utf8");
    const crc = lib.symbols.nie_crc32(ptr(probe), BigInt(probe.byteLength)) >>> 0;
    return {
      status: {
        loaded: true,
        path: foundPath,
        version,
        crc32SelfTest: crc === 0xcbf43926,
        searchedPaths,
        error: null,
      },
      lib,
    };
  } catch (err) {
    return {
      status: {
        loaded: false,
        path: foundPath,
        version: null,
        crc32SelfTest: null,
        searchedPaths,
        error: err instanceof Error ? err.message : String(err),
      },
      lib: null,
    };
  }
}

async function queryHttpHealth(baseUrl: string) {
  const url = `${baseUrl.replace(/\/+$/, "")}/health`;
  try {
    const res = await fetch(url, { method: "GET", signal: AbortSignal.timeout(1500) });
    const text = await res.text();
    let body: unknown = text;
    try {
      body = JSON.parse(text);
    } catch {}
    return { ok: res.ok, url, status: res.status, body, error: null };
  } catch (err) {
    return {
      ok: false,
      url,
      status: null,
      body: null,
      error: err instanceof Error ? err.message : String(err),
    };
  }
}

function resolveNativeBinary() {
  const binName = process.platform === "win32" ? "nie.exe" : "nie";
  const list: string[] = [];
  if (process.env.NIE_BIN) list.push(process.env.NIE_BIN);
  const onPath = Bun.which("nie");
  if (onPath) list.push(onPath);
  list.push(
    resolve(process.cwd(), "target", "release", binName),
    resolve(process.cwd(), "target", "debug", binName),
    resolve(import.meta.dir, "..", "..", "target", "release", binName),
    resolve(import.meta.dir, "..", "..", "target", "debug", binName),
  );
  const found = list.find((p) => existsSync(p)) ?? null;
  return { found: found !== null, path: found };
}

function runVfsSubcommand(args: string[]) {
  const bin = resolveNativeBinary();
  if (!bin.found || !bin.path) {
    return {
      ok: false,
      command: ["nie", "vfs", ...args],
      binary: bin,
      exitCode: 127,
      stdout: null,
      stderr: "native nie binary not found",
    };
  }
  const vfsArgs = args.length > 0 ? args : ["stats"];
  const proc = Bun.spawnSync([bin.path, "vfs", ...vfsArgs], {
    stdout: "pipe",
    stderr: "pipe",
    env: process.env,
  });
  const stdoutText = proc.stdout.toString("utf8").trim();
  const stderrText = proc.stderr.toString("utf8").trim();
  let parsedStdout: unknown = stdoutText;
  if (stdoutText.startsWith("{") || stdoutText.startsWith("[")) {
    try {
      parsedStdout = JSON.parse(stdoutText);
    } catch {}
  }
  return {
    ok: proc.exitCode === 0,
    command: [bin.path, "vfs", ...vfsArgs],
    binary: bin,
    exitCode: proc.exitCode,
    stdout: parsedStdout,
    stderr: stderrText || null,
  };
}

function emit(payload: unknown, exitCode = 0): never {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exit(exitCode);
}

async function main() {
  const [cmd = "status", ...rest] = process.argv.slice(2);
  switch (cmd) {
    case "status": {
      const { status: abi, lib } = openNieAbi();
      lib?.close();
      const http = await queryHttpHealth(DEFAULT_HTTP_URL);
      const nativeBinary = resolveNativeBinary();
      emit(
        {
          ok: true,
          command: "status",
          runtime: { bun: Bun.version, platform: process.platform, arch: process.arch },
          abi,
          http,
          nativeBinary,
        },
        0,
      );
    }
    case "health": {
      const http = await queryHttpHealth(rest[0] ?? DEFAULT_HTTP_URL);
      emit({ command: "health", ...http }, http.ok ? 0 : 1);
    }
    case "vfs": {
      const result = runVfsSubcommand(rest);
      emit({ command: "vfs", ...result }, result.ok ? 0 : result.exitCode || 1);
    }
    case "abi": {
      const { status, lib } = openNieAbi();
      if (rest[0] === "crc32") {
        const input = rest.slice(1).join(" ");
        if (!lib) emit({ ok: false, command: "abi", action: "crc32", abi: status }, 1);
        const buf = Buffer.from(input, "utf8");
        const crc = lib.symbols.nie_crc32(ptr(buf), BigInt(buf.byteLength)) >>> 0;
        lib.close();
        emit(
          {
            ok: true,
            command: "abi",
            action: "crc32",
            input,
            crc32: crc,
            hex: `0x${crc.toString(16).toUpperCase().padStart(8, "0")}`,
            abi: status,
          },
          0,
        );
      }
      lib?.close();
      emit({ ok: true, command: "abi", abi: status }, 0);
    }
    default:
      emit(
        {
          ok: false,
          error: `unknown subcommand: ${cmd}`,
          usage: ["status", "health [url]", "vfs [args...]", "abi [crc32 <text>]"],
        },
        2,
      );
  }
}

void main();
