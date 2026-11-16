# Recon map: inazuma.jp (Inazuma Eleven series portal)

Read on 2026-11-16, one page at a time with agent-browser. The 22 section pages listed in `screens/index.tsv` were opened and captured full page in `screens/` (reference only, never shipped). The site has no sitemap.xml (404). Sections were taken from the links on the home page. Inside a template, only one representative page was read: there are 70 patch notes, 32 movies and 16 tournament pages on the same templates.

## Scope
- **App and platform**: the inazuma.jp website (responsive web) and its encyclopaedia zukan.inazuma.jp (Inagle).
- **Slice**: portal, game pages, Victory Road hub (characters, patch notes, tournaments, competition, FAQ, specials, movies), films and events.
- **For whom**: the Rose Griffon community, as a multilingual (en, fr, ja) self-hostable Rust site. Game data is read by nie from the player's own install.

## Sources
| Source | URL |
| --- | --- |
| Portal | https://www.inazuma.jp/ |
| Victory Road hub | https://www.inazuma.jp/victory-road/ |
| Patch notes (ja and en toggle) | https://www.inazuma.jp/victory-road/patchnote/ps-steam/ |
| Encyclopaedia | https://zukan.inazuma.jp/ |

## Screens
| ID | Screen | Route | Purpose | Key components | Links seen |
| --- | --- | --- | --- | --- | --- |
| S01 | Portal home | / | entry to every title and campaign | key visual, anniversary banner, topics ticker, banner grid, featured title, social tiles, title card grid with platform chips | 344 |
| S02-S10 | Title page (one per game) | /inazuma/ ... /re/ | presentation of one game | hero, story, features, platforms, purchase links | 9-87 |
| S11 | Victory Road home | /victory-road/ | game hub | sticky top bar (LEVEL5, X, Facebook, series link, language), MENU drawer, buy button, news, section tiles, footer nav | 127 |
| S12 | Characters | /victory-road/character/ | browse characters by school | school cards (emblem, name in ja and romaji, three portraits), main and select groups | 70 |
| S13 | Movies | /victory-road/movie/ | trailers | video grid, player modal | 105 |
| S14 | Patch notes | /victory-road/patchnote/ps-steam/ | updates per platform | platform tabs, ja and en toggle, list rows (date JST, version title, platform chips), NEW badge | 92 |
| S15 | Online tournament | /victory-road/online-tournament/ | tournament system | rules sections, rank tiers, rewards | 96 |
| S16 | Competition | /victory-road/competition/ | online play modes | mode cards, rules | 102 |
| S17 | Heroes showcase | /victory-road/heroes-showcase/ | launch presentation | video, summary | 14 |
| S18 | FAQ | /victory-road/guide/faq/ | support | category list, collapsible questions | 63 |
| S19 | Specials | /victory-road/special/ | campaigns | campaign cards | 69 |
| S20 | Films | /movie/ | theatrical films | hero, streaming links, topics | 62 |
| S21 | Event | /event/ | event | schedule, venue | 42 |
| S22 | Film archive | /archive/eiga/01/ | past film | single page | 5 |

## Flows
- **U01 Find a game**: S01, then a title card, then S02-S11. 1 click.
- **U02 Read the latest update for my platform**: S11, then patch notes, then the platform tab (S14), then the version row. 3 clicks. Edge cases: English only for some notes, versions per platform differ.
- **U03 Look up a character**: S11, then characters (S12), then a school, then a character. 3 clicks. On zukan, the stats and skills page.
- **U04 Join a tournament**: S11, then online tournament (S15), then the rules and rank tiers. External sign-up.
- **U05 Watch a trailer**: S13, then the video modal.

## Components
| Component | Variants | Used on |
| --- | --- | --- |
| Top bar | brand, social icons, language menu | S11-S19 |
| Menu drawer | sections of the game | S11-S19 |
| Hero with title (Latin display plus ja subtitle) | | S12-S19 |
| Topics list | date and title, ticker | S01, S11 |
| Banner tile | image, CTA arrow | S01, S11, S19 |
| Title card | key art, platform chips, name | S01 |
| Platform chip | Switch 2, Switch, PS5, PS4, Xbox, Steam, iOS/Android | S01, S14 |
| List row with date, title and chips | NEW badge | S14 |
| School card | emblem, names in two scripts, portraits | S12 |
| Language toggle | ja, en | S14 |
| Footer nav | game sections | S11-S19 |
| Floating buy button | | S11-S19 |

## Inferred data model
- **Game** (slug, title per language, platforms[], official_url): evidence S01 title cards. Confidence: high.
- **Platform** (code, label): evidence the chips on S01 and S14. Confidence: high.
- **Topic/News** (date, title, game, link): evidence S01 topics. Confidence: high.
- **PatchNote** (game, platform[], version, date JST, body, lang): evidence S14. Confidence: high.
- **School/Team** (name ja, romaji, emblem, members[]): evidence S12. Matches the team tables in cfg.bin. Confidence: high.
- **Character** (name, school, portrait, stats, skills): evidence S12 and zukan. Data comes from cfg.bin. Confidence: high.
- **Tournament** (season, rank tiers, rewards, rules): evidence S15. Matches achillea-store. Confidence: medium.
- **Video** (title, url, date): evidence S13. Confidence: high.

## Not cloned (skip)
- Key art, character illustrations, logos, videos and music. These are Level-5 assets. Visuals come from our tokens, and game images come from the player's own install through nie.
- Store and purchase links, the official tournament sign-up, and the X and LINE accounts.
- The official text. News and patch notes are written by us, and we link to the official pages.

## Size
22 screen templates, 5 flows, 8 entities. Hard parts: names and text per language from the VFS, patch notes per platform, multiplatform data. Size: M.
