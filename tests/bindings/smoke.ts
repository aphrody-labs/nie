// Bun FFI binding smoke test: abi version and cfgbin_to_sql on a synthetic fixture.
import { readFileSync } from "node:fs";

const { openNieFfi, NIE_FFI_ABI_VERSION } = await import(`${process.env.NIE_BINDINGS}/bun/nie_contract.ts`);
const api = openNieFfi(process.env.NIE_LIB as string);
const sql = api.cfgbinToSql(new Uint8Array(readFileSync(process.argv[2])), 1);
if (sql === null || !sql.includes("CREATE TABLE") || !sql.includes("ROOT")) throw new Error(`bad sql: ${sql}`);
if (api.cfgbinToSql(new TextEncoder().encode("nope"), 1) !== null) throw new Error("garbage accepted");
console.log("bun ok", NIE_FFI_ABI_VERSION, sql.length);
api.close();
