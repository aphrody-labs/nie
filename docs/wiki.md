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
   capsule lot contents (their `vars[1]` matches no table or text), match lineups and invocation
   rates. Team affiliation does exist: `chara_base` variable 16 (belongTeamId) points to a
   `belong_team_config` entry (6100 of 6161 characters, 199 of 208 teams).
4. No hardcoded counts or versions: counts come from the decoded tables, the game version from the
   installation.
5. Text in the nine languages the VFS ships (`de en es fr it ja pt zh_hans zh_hant`, BCP 47
   `zh-Hans`, `zh-Hant`). No machine translation is presented as the game's text.
6. Every page names the VFS path and table that back it.

## Where it runs

The crate `crates/nie-wiki-vfs` computes every record (`Wiki`) and renders the HTML pages. Surfaces:

- CLI: `nie wiki characters|character <key>|skills|skill <key>|teams|team <id>|languages|unsourced`
  with `--lang de,fr`, `--json`, `-q`, `--limit`, `--offset` (MCP tool `cli_wiki`).
- `nie serve`: JSON under `/v1/wiki/...`, HTML under `/wiki`, described by `/openapi.json`
  (see [serve.md](serve.md)).

How the values are read:

- Character: `chara_param` (element, positions, growth pattern, learned skills) joined to
  `chara_base` (code, name, family name, description, gender, series id, team id). Gender is the
  stored code (1 = male, 2 = female; the table also holds 0, 4 and 5 for props, an NPC and
  monsters). Positions are the stored `chara_param` codes; the VFS has no label for them.
- Names are `NOUN_INFO` rows of `chara_text`, keyed by (hash, form): form 0 is the full name, 11 and
  12 its two parts. Descriptions are `TEXT_INFO` rows. Every form of every language is served.
- Element labels are the `system_text` rows keyed by `crc32("element_type_<name>")`, the way the
  game hashes its enumeration keys.
- Skill: all fields of a `skill_config` entry as stored, with the name (`skillNameId`) and
  description (`skillDescId`) from `skill_text`.
- Team: a `belong_team_config` entry, the name from `team_text`, each `teamEmblemId_*` resolved by
  hashing the CPK file names (`em010001.g4tx` hashes to `0xD5ABE1F0`); an id no file name hashes
  to stays unresolved.
- The languages are the folders of `data/common/text` that hold `chara_text.cfg.bin`.

## Sources

The page families and their tables are listed in the VFS column of rosegriffon's
`docs/research/inazuma-ecosystem-parity.md`. The rosegriffon tag `legacy-azalee-wiki`
(commit `e4464109`, `apps/rg-website/src/app/wiki/**`) is a UX and feature reference only, never a
data source. `crates/nie-wiki` and `crates/nie-zukan` read that legacy mirror and are replaced, not
extended.

## What the VFS labels and what it does not

Measured on the installation by `crates/nie-wiki-vfs` (example `pages`):

- Names: `chara_text` `NOUN_INFO` is keyed by (hash, form): form 0 full name, 11 family name,
  12 given name. All forms are kept.
- Element is the only code with a text label: `system_text` key
  `crc32("element_type_<wind|forest|fire|mountain>")`. "none" and "void" have no text row.
- Position (`chara_param` variable 3), gender (`chara_base` variable 11: 1 male, 2 female; 0, 4
  and 5 are props, an NPC and monsters), skill category and series id have no label in any of the
  5871 gamedata files: they are served as stored codes.
- 206 of 208 team emblems resolve; `0x86D317FE` and `0x339AD19E` match no CPK file name.
- `data/common/gamedata/team/team_config_*.cfg.bin` (`TEAM_MEMBER_SKILL_CONFIG_INFO_LIST_BEG`,
  9073 rows of 13 integers) is not decoded yet.

## Hosting

Public at https://nie.aphrody.app (own stack, see `docs/serve.md`), textures and models included,
by owner decision on 2026-11-19. The Level-5 partner admin reviews licensing after the deploy.
