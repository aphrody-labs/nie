# tools/zukan

Moved from `rose-griffon/rosegriffon` (`packages/inagle`, 2026-11-19) when the Inazuma Eleven domain left that repository.
Only what nie does not already cover was kept; the rest of `packages/inagle` (parsers, cfg.bin, CPK, SQL layer, CLI) is
duplicated by `nie-data`, `nie-formats`, `nie-cfgsql` and `nie-wiki-vfs` and stays in the Rose Griffon archive bundle.

- `src/` Zukan scraper, matcher, audit and VFS fusion (zukan.inazuma.jp); `src/crand.ts` pure-TS `lives::CRand` (MT19937).
- `scripts/` crawlers (patch notes, topics, zukan) and audit scripts.
- `data/` `games.toml` (25 eras of the saga), `official-topics.json`, `re-live.json`, `series-live.json`, `zukan-audit.json`.
- `sql/` `20261001_inagle_wiki_projections.sql`, `20260605_event_subtitles.sql`.

Status: ported as is. Imports of `@rosegriffon/db` and `@rosegriffon/inagle` are not rewired yet; target is a Rust plugin
(`plugins/zukan`) on `nie-cfgsql` and `nie-sql`. Licensing of scraped content goes to the Level-5 partner review.
