# Wiki: the game's data, read from the VFS

The Inazuma Eleven: Victory Road wiki lives in nie. It was on rosegriffon.fr until 2026-11-19, when
Level-5 asked for a more factual version; rosegriffon.fr now keeps community content only and links
here. Every page is computed from the installation's VFS at request time.

## Rules

1. Every field is a value from a VFS table (`data/common/gamedata/**.cfg.bin`) or text file
   (`data/common/text/<lang>/*.cfg.bin`), served as the game value plus its decoded label. A field
   with no source is absent. No derived copies: the legacy `inagle_*` SQLite mirror had gender
   codes inverted (the game uses 1 = male, 2 = female), position "Inconnu" on 6101 of 10079 rows
   while the real code sat in the raw record, and categories in mixed casing.
2. References resolve the way the game does. A team's `emblemId` is the CRC32 of the emblem file
   name (`crc32("em010001") = 0xD5ABE1F0`); resolve it by hashing the real CPK file names, never
   through a hand-made table.
3. Where no source exists, the page shows an explicit empty state. No source is known today for
   capsule lot contents (their `vars[1]` matches no table or text), team rosters (no
   character-to-team link in the tables read so far) and invocation rates.
4. No hardcoded counts or versions: counts come from the decoded tables, the game version from the
   installation.
5. Text in the nine languages the VFS ships (`de en es fr it ja pt zh_hans zh_hant`, BCP 47
   `zh-Hans`, `zh-Hant`). No machine translation is presented as the game's text.
6. Every page names the VFS path and table that back it.

## Sources

The page families and their tables are listed in the VFS column of rosegriffon's
`docs/research/inazuma-ecosystem-parity.md`. The rosegriffon tag `legacy-azalee-wiki`
(commit `e4464109`, `apps/rg-website/src/app/wiki/**`) is a UX and feature reference only, never a
data source. `crates/nie-wiki` and `crates/nie-zukan` read that legacy mirror and are replaced, not
extended.

## Open questions for the Level-5 partner review

- Whether the wiki is served on a public host, and whether it may include textures and models
  (rosegriffon's CDN routes for game assets now answer 410).
