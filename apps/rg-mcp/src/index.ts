#!/usr/bin/env bun
// SPDX-License-Identifier: Apache-2.0
/**
 * Rose Griffon MCP server. Every tool is generated from the operation registry of
 * `@rosegriffon/ops`; this file only picks the transport.
 *
 *   bun apps/mcp/src/index.ts           stdio (the `rosegriffon` entry of .mcp.json); caller = local operator
 *   bun apps/mcp/src/index.ts --http    streamable HTTP on RG_OPS_HOST:RG_OPS_PORT at /mcp, together with
 *                                       A2A (/a2a, /.well-known/agent-card.json) and REST (/ops);
 *                                       callers are authenticated by bearer token (RG_OPS_ADMIN_TOKEN,
 *                                       RG_OPS_STAFF_TOKEN), anonymous callers get public operations only
 */
import { createContext, createDefaultRegistry } from "@rosegriffon/ops";
import { startOpsServer } from "@rosegriffon/ops/http";
import { MCP_SERVER_INFO, serveMcpStdio } from "@rosegriffon/ops/mcp";

export const SERVER_NAME = MCP_SERVER_INFO.name;
export const SERVER_VERSION = MCP_SERVER_INFO.version;

async function main(): Promise<void> {
  const registry = createDefaultRegistry();
  if (process.argv.includes("--http")) {
    const ctx = createContext({ surface: "rest", role: "public" });
    const server = startOpsServer({ registry, ctx });
    console.error(
      `[rg-ops] ${registry.size} operations on ${server.url} (mcp: /mcp, a2a: /a2a, agent card: /.well-known/agent-card.json, rest: /ops)`,
    );
    return;
  }
  serveMcpStdio(registry, createContext({ surface: "mcp" }));
}

if (import.meta.main) {
  await main();
}
