# Wiki: the game's data, read from the VFS

The Inazuma Eleven: Victory Road wiki lives in nie. Level-5 asked for a factual version, so every page is computed from the installation's VFS at
request time.

## Rules

1. Every field is a value from a VFS table (`data/common/gamedata/**.cfg.bin`) or text file
   (`data/common/text/<lang>/*.cfg.bin`), served as the game value plus its decoded label. A field
   with no source is absent. No derived copies: the legacy `inagle_*` SQLite mirror had gender
   codes inverted (the game uses 1 = male, 2 = female), position "Inconnu" while
   the real code sat in the raw record, and categories in mixed casing.
2. References resolve the way the game does. A team's `emblemId` is the CRC32 of the emblem file
   name (`crc32("em010001") = 0xD5ABE1F0`); resolve it by hashing the real CPK file names, never
   through a hand-made table.
3. Where no source exists, the page shows an explicit empty state. No source is known today for
   capsule lot contents (their `vars[1]` matches no table or text), match lineups and invocation
   rates. Team affiliation does exist: `chara_base` variable 16 (belongTeamId) points to a
   `belong_team_config` entry (not every character or team has one).
4. No hardcoded counts or versions: counts come from the decoded tables, the game version from the
   installation.
5. Text in the nine languages the VFS ships (`de en es fr it ja pt zh_hans zh_hant`, BCP 47
   `zh-Hans`, `zh-Hant`). No machine translation is presented as the game's text.
6. Every page names the VFS path and table that back it.

## Where it runs

The crate `crates/nie-wiki-vfs` computes every record (`Wiki`) and renders the HTML pages. Surfaces:

- CLI: `nie wiki characters|character <key>|skills|skill <key>|teams|team <id>|tables|table <table>
  <class>|catalog|languages|unsourced|verify` with `--lang de,fr`, `--json`, `-q`, `--class`,
  `--limit`, `--offset` (MCP tool `cli_wiki`).
- `nie serve`: JSON under `/v1/wiki/...`, HTML under `/wiki`, described by `/openapi.json`
  (see [serve.md](serve.md)).

How the values are read:

- Characters: every `chara_param` entry and every `chara_base` entry is one record, never dropped.
  A param joins the first base entry of its baseId (`param_and_base`); a param whose baseId names
  no base entry keeps its own fields (`param_without_base`); a base entry no param names, or a
  repeated id, is a record without param fields (`base_without_param`). Counts come from the
  tables and are shown on the pages (`classes`).
- Gender is the stored code (1 = male, 2 = female; the table also holds 0, 4 and 5 for props, an
  NPC and monsters). Positions are the stored `chara_param` codes; the VFS has no label for them.
  `__SORT_INDEX` and `CHARA_BASE_INFO_REF_BATTLE` entries are attached to the record of the same
  position (`sort_index`, `battle`).
- Names are `NOUN_INFO` rows of `chara_text`, keyed by (hash, form): form 0 is the full name, 11 and
  12 its two parts; `chara_base` variables 3, 4 and 5 are the keys of the name, the family name and
  a second family-name text. `chara_text_roma` rows of the same keys are the romanized names.
  Descriptions are `TEXT_INFO` rows. Every form of every language is served.
- Series: `chara_base` variable 15 names a `chara_series_config` entry whose
  `charaSeriesNameTextId` is a `chara_add_info_text` row.
- Learned skills: the ids of a `chara_param` entry name an entry of `skill_config` (skillID),
  `aura_skill_config` (`AURA_CMD_INFO` variable 0) or `passive_skill_config` (`PASSIVE_SKILL_INFO`
  variable 0), whose name key is a `skill_text` row. An id in none of the three, an entry with no
  name key and a key no language holds are each shown as such.
- Element labels are the `system_text` rows keyed by `crc32("element_type_<name>")`, the way the
  game hashes its enumeration keys.
- Skill: all fields of a `skill_config` entry as stored, with the name (`skillNameId`) and
  description (`skillDescId`) from `skill_text`.
- Team: a `belong_team_config` entry, the name from `team_text`, each `teamEmblemId_*` resolved by
  hashing the CPK file names (`em010001.g4tx` hashes to `0xD5ABE1F0`); an id no file name hashes
  to stays unresolved.
- Tables: every node class (T2B) or list (RDBN) of `chara_param`, `chara_base`,
  `chara_series_config`, `seasonal_chara_param`, `chara_param_table_config`, `skill_config`,
  `passive_skill_config`, `aura_skill_config`, `change_aura_skill_config`,
  `override_skill_config`, `real_skill_config` and `belong_team_config` is served as stored. A T2B
  file stores positional variables with no field names, so they are served positionally; text
  references are labelled where a table's variable is verified to hold text keys (`name`,
  `description`). In a T2B container (`X_LIST_BEG`) the nodes named `__SORT_INDEX` and `X_REF_Y`
  are annotations of the records of `X`: entry k belongs to record k, and `X_REF_Y` holds the
  `[offset, count]` slice of the class `P_Y` (the longest prefix `P` of `X` that has one).
- The catalog lists every file of `data/common/gamedata` and whether a table above reads it; the
  files marked `present` are not decoded.
- The languages are the folders of `data/common/text` that hold `chara_text.cfg.bin`.

## Conformance

`nie wiki verify [--server <url>]` compares what a wiki serves to the VFS. The raw side reads the
cfg.bin and text files with the `nie-formats` parsers directly. For every record of every family
(not a sample) it rebuilds the expected record from the raw entry and reports each missing field,
extra field and differing value: the characters, skills, teams, every table class, the table
list, the catalog and the languages. It also fetches every page: the list pages must link every
record, and each record page's stated provenance (path, table, index) must name an entry whose
stored variables or fields equal what the page shows. It exits non-zero on any mismatch. Facts
about the data that the wiki already shows as empty states (a skill with no name text key) are
printed as notes, not counted as mismatches. Without `--server` the same route handlers answer in
process; with it the routes of a running `nie serve` are fetched. The unit test builds a synthetic
installation and requires zero mismatches.

## Sources

The page families and their tables come from the VFS only: `nie vfs formats` and `nie wiki` list
them. No external mirror is a data source.

## What the VFS labels and what it does not

Measured on the installation by `nie wiki`:

- Names: `chara_text` `NOUN_INFO` is keyed by (hash, form): form 0 full name, 11 family name,
  12 given name. All forms are kept.
- Element: `system_text` key `crc32("element_type_<wind|forest|fire|mountain>")`. "none" and
  "void" have no text row. Series have a name through `chara_series_config`.
- Position (`chara_param` variable 3), gender (`chara_base` variable 11: 1 male, 2 female; 0, 4
  and 5 are props, an NPC and monsters) and skill category have no label in any of the gamedata
  files: they are served as stored codes.
- Two team emblem ids, `0x86D317FE` and `0x339AD19E`, match no CPK file name.
- `data/common/gamedata/team/team_config_*.cfg.bin` (`TEAM_MEMBER_SKILL_CONFIG_INFO_LIST_BEG`,
  rows of 13 integers) is listed in the catalog as present and not decoded.

## Hosting

Public at the hostname configured on the tunnel (own stack, see `docs/serve.md`), textures and models included,
by owner decision on 2026-11-19. The Level-5 partner admin reviews licensing after the deploy.
