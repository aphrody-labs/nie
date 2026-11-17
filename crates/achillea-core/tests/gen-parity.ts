// Generates tests/parity.json by running the TypeScript implementation (packages/achillea-core).
// Run: bun crates/achillea-core/tests/gen-parity.ts
import { writeFileSync } from "node:fs";

import * as bracketsMod from "../../../packages/achillea-core/src/brackets/index.ts";
import * as clansMod from "../../../packages/achillea-core/src/clans/index.ts";
import * as eloMod from "../../../packages/achillea-core/src/elo/index.ts";
import * as leagueMod from "../../../packages/achillea-core/src/league/index.ts";
import * as rankedMod from "../../../packages/achillea-core/src/ranked/index.ts";

let seed = 123456789;
const rnd = () => {
  seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
  return seed / 2 ** 32;
};
const int = (lo: number, hi: number) => lo + Math.floor(rnd() * (hi - lo + 1));

const out: Record<string, unknown[]> = {};
const push = (k: string, v: unknown) => (out[k] ??= []).push(v);

const ratings = [0, 100, 399, 400, 750, 1000, 1500, 2000, 2400];
for (const p of ratings)
  for (const o of ratings) {
    push("expected", { p, o, r: eloMod.expectedScore(p, o) });
    for (const a of [0, 0.5, 1]) {
      push("delta", { p, o, a, k: 32, r: eloMod.eloDelta(p, o, a, 32) });
      push("delta", { p, o, a, k: 20, r: eloMod.eloDelta(p, o, a, 20) });
      for (const t of [-1, 0, 1, 3, 5, 8, 10, 11, 99, 3.7])
        push("progressive", { p, o, a, t, k: 32, r: eloMod.progressiveEloDelta(p, o, a, t, 32) });
      push("progressive", { p, o, a, t: 4, k: 48, r: eloMod.progressiveEloDelta(p, o, a, 4, 48) });
    }
    if (p !== o) push("match", { w: p, l: o, k: 32, r: eloMod.computeMatchElo(p, o, 32) });
  }
for (const s of [0, 1, 8, 9, 16, 17, 32, 33, 64, 65, 128, 129, 1000])
  for (const rk of [0, 1, 2, 3, 4, 5, 100])
    push("prize", { s, rk, r: eloMod.placementPrizeAp(s, rk) });

for (const [b, w] of [[100, 0], [100, 4999], [100, 5000], [100, 12345], [50, -10]])
  push("window", { b, w, r: rankedMod.eloWindow(b, w) });
for (const v of [-5, 0, 3.9, 99, 100, 5000, 7])
  push("clamp", { v, r: rankedMod.clampScore(v) });
for (const v of [2, 4, 3, 0, 4.5])
  push("mode", { v, r: rankedMod.parseMode(v) });

for (let i = 0; i < 200; i++) {
  const n = int(0, 7);
  const q = Array.from({ length: n }, (_, k) => ({
    userId: rnd() < 0.15 ? "dup" : `u${k}`,
    elo: int(0, 600),
    enqueuedAt: int(0, 20000),
  })).sort((a, b) => a.elo - b.elo);
  const now = 20000 + int(0, 30000);
  push("queue", { q, now, base: 100, r: rankedMod.selectQueuePair(q, now, 100) });
}
for (let i = 0; i < 300; i++) {
  const mk = (id: string) => ({ userId: id, myScore: int(0, 5), oppScore: int(0, 5), submitted: rnd() < 0.7 });
  const home = mk("h");
  const away = rnd() < 0.5 ? { userId: "a", myScore: home.oppScore, oppScore: home.myScore, submitted: rnd() < 0.8 } : mk("a");
  const forced = rnd() < 0.5;
  push("scores", { home, away, forced, r: rankedMod.resolveScores(home, away, forced) });
}

for (let n = 0; n <= 70; n++) {
  push("brackets", {
    n,
    bye: bracketsMod.byeCount(n),
    se: bracketsMod.singleEliminationRounds(n),
    de: bracketsMod.doubleEliminationRounds(n),
    rr: bracketsMod.roundRobinRounds(n, false),
    rr2: bracketsMod.roundRobinRounds(n, true),
    mc_se: bracketsMod.stageMatchCount("single_elimination" as never, n),
    mc_de: bracketsMod.stageMatchCount("double_elimination" as never, n),
    mc_rr: bracketsMod.stageMatchCount("round_robin" as never, n),
    mc_rr2: bracketsMod.stageMatchCount("round_robin" as never, n, true),
  });
}
for (const s of [1, 2, 3, 4, 5, 8, 12, 16, 31, 32, 33, 64])
  push("seeds", { s, r: bracketsMod.standardSeedOrder(s) });

for (let n = 2; n <= 9; n++) {
  const ids = Array.from({ length: n }, (_, i) => `t${i}`);
  push("fixtures", { ids, r: leagueMod.generateSingleRoundRobin(ids) });
}
for (let i = 0; i < 60; i++) {
  const n = int(3, 7);
  const ids = Array.from({ length: n }, (_, k) => `t${k}`);
  const fx = leagueMod.generateDoubleRoundRobin(ids);
  const results = fx
    .filter(() => rnd() < 0.8)
    .map((f) => {
      const hs = int(0, 4);
      let as = int(0, 4);
      if (hs === as) as = hs + 1;
      return { homeId: f.homeId, awayId: f.awayId, homeScore: hs, awayScore: as, winnerId: null, overtime: rnd() < 0.3 };
    });
  for (const cfg of ["asytral", "football"] as const) {
    const c = cfg === "asytral" ? leagueMod.ASYTRAL_POINTS : leagueMod.FOOTBALL_POINTS;
    push("table", { ids, results, cfg, r: leagueMod.computeLeagueTable(ids, results as never, c) });
  }
}

for (const d of ["Staff", "Joueur", "Player", "Artiste", "Artist", "Créateur de contenu", "Content Creator", "ContentCreator", "x"])
  push("distinction", { d, r: clansMod.distinctionPriority(d) });
for (let a = 0; a <= 3; a++)
  for (let b = 0; b <= 3; b++)
    push("clanrole", {
      a,
      b,
      atLeast: clansMod.hasClanRoleAtLeast(a, b),
      act: clansMod.canActOnMember(a, b),
      recruit: clansMod.canManageRecruitment(a),
      dissolve: clansMod.canDissolveClan(a),
    });

writeFileSync(new URL("./parity.json", import.meta.url), JSON.stringify(out));
console.log(Object.fromEntries(Object.entries(out).map(([k, v]) => [k, v.length])));
