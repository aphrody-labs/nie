# Inazuma Eleven ecosystem: feature-parity matrix

Date: 2026-11-19 (server `Date` headers 01:12 to 01:21 UTC). Scope: https://www.inazuma.jp, https://zukan.inazuma.jp, https://www.level5.co.jp, https://www.inazuma-cross.jp (linked from both), plus the community ecosystem.
Method: about 110 serial GETs, `User-Agent: rosegriffon-research/1.0 (+https://rosegriffon.fr)`, one request at a time per host with a 1 s gap, no cookies, no logins, no media downloaded. Builds on `docs/research/inazuma-jp-level5-endpoints.md` (2026-11-17); this file only restates what changed or was newly verified. Repo facts come from reading `packages/azalee`, `packages/inagle`, `packages/cron`, `packages/i18n`, `apps/rg-website`, `infra/services.toml` on `main`. Nothing was edited outside `docs/research/`.

Legend. Source: O official, C community. Data source: PAGE official HTML page, API official JSON/XML endpoint, VFS game files (IEVR `data/common/**` via the nie engine of Aphrody Labs), COMM community site/API. Target: PROXY (edge cache of official origin, normalised metadata), VFS (derived from the game, no official fetch), LINK (link out only). Legal flag `L!` means the row would republish official assets or text beyond metadata or cache; the decision belongs to the Level-5 partner admin, not to agents (AGENTS.md).

## 1. Findings that change the plan

1. No official sitemap exists for the three Level-5 portals. Verified live: `robots.txt` and `sitemap.xml` return 404 (HTML 404 page, `<meta name="robots" content="noindex">` on inazuma.jp) on `www.inazuma.jp`, `inazuma.jp`, `www.level5.co.jp` and `zukan.inazuma.jp`; `sitemap.html` is 404 too. The only sitemaps are on `www.inazuma-cross.jp` (section 2.4). `lastmod`-driven freshness is therefore possible for Cross only; the other hosts need link-graph discovery plus content hashes.
2. There is no official JSON API on any of these hosts. The only dynamic call found is `fetch('https://www.inazuma.jp/zukan/assets/inc/topics.html')` on the zukan home (an HTML fragment, empty body at fetch time, 200). zukan scripts (`tile.js`, 1 KB) call nothing. `inazuma-cross.jp/wp-json/` is disabled (401). All "official endpoints" are server-rendered HTML plus the Cross RSS feed.
3. Correction of the earlier note: `zukan.inazuma.jp` serves only two languages live. `/en/chara_list/` returns 200 with `<html lang="en">`; `/fr/`, `/it/`, `/de/`, `/es/`, `/zh-tw/`, `/zh-cn/`, `/pt-br/` all answer 302 to `/` (Japanese), although the page still advertises them via hreflang. `/ko/` is 404. The 9-title appearance flags and `per_page` are as before; `?per_page=200` on GET now verified (202 `data-chara-id` rows in one response), so a full crawl is about 28 requests per language, not 110. `HEAD` still returns 500 on list pages.
4. `www.inazuma.jp/victory-road/<lang>/` is the real multilingual surface: `en fr it de es zh-tw zh-cn pt-br` all 200 (55 to 57 KB each), `ko` 404, top-level `/en/` 404. The localized subtree mirrors the section tree (`character/`, `patchnote/`, `topics/`, `system/`, `story/`, `kizuna/`, `chronicle/`, `product/`, `guide/*`, `guideline/`, `streamer-kit/`, `special/`, `competition/`, `online-tournament/`, `movie/`, `inadai`). `/re/en/` exists (INAZUMA ELEVEN RE). `www.level5.co.jp/blog/{en,zh-tw,zh-cn}/` exist; `/blog/ko/` and `/en/` are 404.
5. Origin caching: `inazuma.jp` and `level5.co.jp` answer `Cache-Control: max-age=300, s-maxage=300` behind IIJ CDN (openresty), no `ETag`, no `Last-Modified`. `inazuma-cross.jp` answers `no-cache`. Zukan sends neither. Conditional GET is unusable except on Cross; hash bodies.
6. `store.inazuma-cross.jp` is a separate Rails shop (robots 200: `Disallow: /admin/`, `/error_*/`, `/assets/`, `/rails/active_storage/`, no sitemap line). Not enumerated; out of scope unless the partner asks.
7. Bold Revolution (sequel, mentioned by `docs/ie/ACTUALITES-IEVR-2026.md` and Wikipedia, announced 2026-09-10) has no page or link on the inazuma.jp or level5.co.jp home pages fetched today (0 matches). UNVERIFIED as an official URL; `packages/inagle/data/series-live.json` lists 14 titles without it.

## 2. Official page families and counts

Counts are page families (distinct URL templates), not URLs. "URLs" are what was observable without crawling every page.

### 2.1 www.inazuma.jp: 15 families

Observed: home TOPICS block 288 `<dl>` entries (newest 2026.11.11), `/victory-road/topics/` lists 29 slugs, home links 27 `ps-steam`, 26 `xbox`, 16 `switch` patch notes, 9 beta patch notes, 3 beta-tournament vote pages.

| # | Family | URL pattern | Approx. size |
|---|---|---|---|
| 1 | Portal home + TOPICS feed | `/` | 288 entries |
| 2 | Portal news | `/topics/<yymmdd>/`, `/movie/topics/<yymmdd>/` | tens |
| 3 | Series microsites | `/{inazuma,inazuma2,inazuma3,inazuma123,inazuma3_rendou,go,go2,gogalaxy,gostrikers,strikers,re,ff,everyday,everyday-plus,inazuma-walker,18th,17th-wallpaper,fes2018,event,archive/{butai,eiga}/NN}/` | about 22 sites |
| 4 | Movie / anime / streaming | `/movie/`, `/movie/streaming/` | 3 pages |
| 5 | Victory Road hub + product | `/victory-road/`, `/victory-road/product/`, `sale/` | 3 pages |
| 6 | VR news | `/victory-road/topics/<yymmdd>[_NN]/`, `topics/inazuma/` | 29 listed, more archived |
| 7 | VR patch notes | `/victory-road/patchnote/{switch,ps-steam,xbox}/ver_<maj>_<min>_<patch>/` | about 69 linked |
| 8 | VR beta test | `/victory-road/beta-test/`, `beta-test/patchnote/ver_*`, `beta-test/tournament/{,vote/,topics/<yymmdd>/}` | about 25 |
| 9 | VR characters by team | `/victory-road/character/{raimon,teikoku,nagumohara,nagumohara-select,hokuyo,tofuikokukan,nishinomiya,chronicle}/` | 8 pages |
| 10 | VR modes and systems | `/victory-road/{system,mode,story,story/story-system,chronicle,kizuna}/` | 6 pages |
| 11 | VR guides and rules | `guide/{faq,cross-save,password}/`, `guideline/`, `tournament_guideline/`, `streamer-kit/` | 6 pages |
| 12 | VR competitions | `competition/`, `online-tournament/`, `heroes-showcase/`, `best-shot/`, `eve/`, `special/` | 6 pages |
| 13 | VR community | `inadai/`, `inadai-<yymmdd>/`, `inazuma-post/`, `inazuma-research/`, `questionnaire/result{,2,3,4}/`, `movie/` | about 12 |
| 14 | VR localized mirror | `/victory-road/<en|fr|it|de|es|zh-tw|zh-cn|pt-br>/<section>/` | 8 langs x about 30 |
| 15 | Zukan topics fragment | `/zukan/assets/inc/topics.html` | 1 (empty today) |

### 2.2 zukan.inazuma.jp (Inagle, official player codex): 9 families

| # | Family | URL | Size / note |
|---|---|---|---|
| 1 | Character list | `/[en/]chara_list/?page&per_page&name_search&gender_filter&attr_filter&pos_filter&grade_filter&team_filter&version_filter&category_filter` | 5,460 rows, 50/100/150/200 per page, 9-title appearance flags |
| 2 | Character stats cards | `/[en/]chara_param/?q=<obf>` | 5,459 cards, 7 stats, Lv50 column, acquisition routes |
| 3 | Special moves | `/[en/]skill/` | 904 moves, per-move `.wmv` + poster |
| 4 | Equipment | `/[en/]item/equip/` | 223 |
| 5 | Fashion | `/[en/]item/fashion/` | linked, not counted |
| 6 | Emblems | `/[en/]item/emblem/` | linked, not counted |
| 7 | 3D viewer | `/chara_model_view/?q=<obf>` | JS viewer |
| 8 | Team builder | `/my_team/` (30 slots, "0/30 Players") | tool |
| 9 | Formation tool | `/soccer_formation/` | tool; `/team_info/` 302 to `/` |

`q` is `base64url(bytes(JSON) XOR 0xFF)` (previous note); prefer plain GET params. Assets: `https://dxi4wb638ujep.cloudfront.net/1/<x>/<y>/<z>/<id>.{png,webp,jpg,wmv}` and UI under `/1/prd/assets/4/`.

### 2.3 www.level5.co.jp: 10 families

| # | Family | URL | Note |
|---|---|---|---|
| 1 | Corporate home | `/`, `/top.html` | game list, TV anime, pickup |
| 2 | News | `/news/<yyyymmdd>[_NN]/` | no index; 6 slugs linked from the inazuma.jp home, 1 from the level5 home |
| 3 | Studio blog (ja + 3 langs) | `/blog/`, `/blog/<yymmdd>/`, `/blog/{en,zh-tw,zh-cn}/` | Hino dev blog |
| 4 | Inazuma dev blog | `/blog_inazuma/` | "5つ星修練場", 200 |
| 5 | Product catalogue | `/products/index.html`, `/products/<franchise>/*.html` | about 80 pages, 12 Inazuma (`/products/inazuma/{inazumaeleven,inazumaeleven2,inazumaeleven3,inazuma123,strikers,strikers2012x,go,go2,gogalaxy,gostrikers,everyday-plus,victory-road}.html`) |
| 6 | Events | `/tgs<year>/`, `/vision<year>/`, `/vision2026-2/`, `/tgs2023/report/<mmdd>/` | per-event microsites |
| 7 | CCC | `/ccc/`, `/ccc/movie/` | creative community club |
| 8 | Ambassador | `/ambassador/`, `/ambassador/{member,movie}/` | `/ambassador/` returns 280 B |
| 9 | Recruit | `/recruit/`, `/new-office/` | off-topic for the hub |
| 10 | Outbound franchise sites | youkai-watch.jp, layton.jp, ninokuni.jp, snack-world.jp, etc. | out of scope, linked only |

### 2.4 www.inazuma-cross.jp (WordPress, mobile game): 6 families (the only host with sitemaps)

`robots.txt` (200) lists `https://website-origin.inazuma-cross.jp/{wp-sitemap.xml,sitemap.xml,sitemap.html}`; the origin host is 403, the public `www` host serves the same files. `/sitemap.xml` is a Google XML Sitemaps v4.1.25 index, generated 2026-11-19: `sitemap-misc.xml` (2 URLs, lastmod 2026-11-18T11:17:37Z), `post-sitemap.xml` (261 URLs: 188 `/information/news/<id>/`, 69 `/information/important/<id>/`, 4 `/information/other_posts/<id>/`; lastmod 2026-11-18T11:17:37Z), `page-sitemap.xml` (16 pages: `terms privacy guidelines guide faq parents trade tutorial-gacha samuraiblue2026 victory-road-2026 cbt-news cbt-report contact{,/form,/thanks} app-redirect`; lastmod 2026-11-17T05:00:43Z). `/wp-sitemap.xml` is an empty `<urlset>`. `/feed/` RSS 2.0 with `content:encoded`, 10 items.

Families: (1) news, (2) important notices, (3) other posts, (4) static pages (terms, guideline, parents, trade, faq, guide), (5) campaign pages (`samuraiblue2026`, `victory-road-2026`, `cbt-*`), (6) RSS feed + sitemap.

Total official families: 15 + 9 + 10 + 6 = 40 (plus the Rails shop, not enumerated).

## 3. Community ecosystem (live sources)

| Site | URL | Offers | Check |
|---|---|---|---|
| Inazuma Eleven Wiki (EN) | https://inazuma-eleven.fandom.com/wiki/Category:Victory_Road_characters | VR characters, teams, moves, series wiki; MediaWiki `api.php` | search result; page 403 to bots (Cloudflare challenge) |
| Inazuma Eleven Wiki (FR) | https://inazuma-eleven.fandom.com/fr/wiki/Inazuma_Eleven_Victory_Road_(Jeu) | FR techniques, passives, supertechniques | search result; `api.php` already in `community-mods.ts` |
| Wiki ES | https://inazuma.fandom.com/es/ | ES wiki | in `community-mods.ts` |
| INAdex | https://inadex.app/ (devlog `/devlog`), https://inazuma-eleven.fr/inadex/ | 5,500+ player DB synced from a sheet, filters, rarity, collection tracker, `/keshin` page | `inazuma-eleven.fr/inadex/` 200 (title "INADEx \| Redirect"); `inadex.app` TLS handshake failed from the research host, UNVERIFIED |
| Inazuma Eleven Hub | https://inazuma-eleven.fr/ | community tools and projects index; mods API `/mods/api/mods` | 200, title "Inazuma Eleven Hub"; SPA, no static links |
| Inazuma Showdown | https://www.inazumashowdown.com/ | competitive community, credits INAdex | 502 at fetch time |
| game-inazuma-eleven | https://github.com/vitorfdl/game-inazuma-eleven | wiki + team builder, about 6,000 players, last push 2026-02-27 | `gh api` |
| ievr_dataminer / ievr_toolbox | https://github.com/Telmo26/ievr_dataminer, https://github.com/Telmo26/ievr_toolbox | datamine to SQLite, one `text/<language>.sqlite` per language | `gh api` last push 2026-02-16 |
| Viola, Mod Manager, Mod Creator | https://github.com/The-RealBobi/Viola, https://github.com/Adr1GR/IEVR_Mod_Manager, https://github.com/Ais2323/Inazuma-Eleven-VR-Mod-Creator | CPK dump, mod install in `data/common/text/<language>/` | search result |
| GameBanana | IEVR PC game 20069, Switch 20068; tools 21299 (save editor), 21993 (InazumaX); mods 638540 (undub), 657415 | mods, tools, undub, FR hissatsu names; `apiv11` | already crawled by `community:mods` (14 games) |
| Nexus Mods | https://www.nexusmods.com/inazumaelevenvictoryroad | mods (VF French mod, Team Editor mod 23) | 403 Cloudflare challenge to bots: LINK only |
| Save editors | https://ievrsaveeditor.com/, https://github.com/adhyayanrathi01/inazuma-eleven-victory-road-save-editor | save/cheat editing (3rd party) | search result; link only |
| atwiki (JA) | https://w.atwiki.jp/inazumavr/ (also `inazuma-v`, `inairevmemo`) | moves, tactics, drop locations, power/tension tables | 200; atwiki pages already listed in `community-mods.ts` for older titles |
| Game8 (JA) | https://game8.jp/inazuma-victory-road | walkthrough, move list | 200 |
| GameCap (JA) | https://gamecap.jp/inazuma-victory-road/waza.html | move shops | search result |
| wikiwiki (JA) | https://wikiwiki.jp/inazuma/ , https://wikiwiki.jp/kikokurimatu/ | JA wiki | in `community-mods.ts` |
| Namu wiki (JA mirror) | https://ja.namu.wiki/ | skill lists | search result |
| TraduSquare, guiasnintendo (ES) | https://tradusquare.es/proyectos/inazuma-eleven-go-galaxy/ | fan translation, guides | in `community-mods.ts` |
| Reddit | https://www.reddit.com/r/inazumaeleven/ | community feed (`new.json`) | `reddit.ts` task exists |
| Tier-list media | TheGamer, Dexerto, Operation Sports, Kongbakpao | rankings | LINK only |
| Rose Griffon / Azalee | https://rosegriffon.fr/wiki | this repo | reference |

## 4. Feature-parity matrix

Rose Griffon (RG) column cites repo paths. "DB" means a table exists but no dedicated route was found under `apps/rg-website/src/app/wiki`. Languages use BCP-47 style: ja, en, fr, de, es, it, pt-BR, zh-Hans, zh-Hant.

### 4.1 Official: www.inazuma.jp

| ID | Page family / feature | Src | URL pattern | Langs | Data source | RG today | Gap | Target | Legal |
|---|---|---|---|---|---|---|---|---|---|
| I1 | Portal home TOPICS feed | O | `/` | ja | PAGE | `packages/inagle/data/official-topics.json` (snapshot 2026-11-14, 5 days stale), cron `crawl` step `news` (`tasks/ie-crawl/news.ts`), site `/news`, `/news/feed.xml`, `/api/news/feed` | `crawl` is MANUAL and `horsServerless`; no 5-min poll, no per-slug body index | PROXY (5 min poll, hash) | metadata only: no flag |
| I2 | Portal news | O | `/topics/<yymmdd>/`, `/movie/topics/<yymmdd>/` | ja | PAGE | covered by `official-topics.json` entries (url + title) | no detail text extract | PROXY | text beyond title: L! |
| I3 | Series microsites (22) | O | `/inazuma*/`, `/go*/`, `/strikers*/`, `/re/`, `/ff/`, `/everyday*/`, ... | ja (+ `/re/en/`) | PAGE | `series.ts` crawl, `series-live.json` (14 titles), `re.ts`, `re-live.json`, `/wiki/jeux` (+`[slug]`), `/api/games` | per-title content pages (story, characters) not mirrored; Bold Revolution absent | LINK + metadata | art: L! |
| I4 | Movie / anime / streaming | O | `/movie/`, `/movie/streaming/` | ja | PAGE | none | no catalogue of films/anime/streaming | LINK | embeds: L! |
| I5 | VR hub / product / editions | O | `/victory-road/`, `product/`, `sale/` | ja + 8 | PAGE | `games-copy.ts` text only | no editions/bonuses/price table | PROXY (metadata) | prices/bonus art: L! |
| I6 | VR news | O | `/victory-road/[<lang>/]topics/<yymmdd>[_NN]/` | ja + 8 | PAGE | `news.ts` fetches `/topics/`, `/en/topics/`, `/fr/topics/` only; `official-topics.json` | 6 of 9 languages not fetched (it, de, es, zh-tw, zh-cn, pt-br) | PROXY | bodies: L! |
| I7 | VR patch notes (3 platforms) | O | `/victory-road/[<lang>/]patchnote/{switch,ps-steam,xbox}/ver_X_Y_Z/` | ja + 8 | PAGE | `/wiki/patch-notes` (+`[id]`), table `patch_notes` (`content_html`, `content_html_fr`), `inagle/scripts/pipeline/sync-patch-notes.ts` | stores full official HTML and a FR translation; no structured change entries; other 7 languages absent; no platform/version diff | PROXY + parse to structured JSON | full HTML copy and machine translation: L! |
| I8 | VR beta test | O | `/victory-road/beta-test/**` | ja | PAGE | none (beta moves appear in community wikis only) | archived beta patch/tournament pages | LINK + metadata | L! if text mirrored |
| I9 | VR characters by team | O | `/victory-road/[<lang>/]character/<team>/` | ja + 8 | PAGE + VFS | `/wiki/chara`, `/wiki/equipe` (VFS-derived `inagle_characters`, `inagle_teams`) | none for data; promo art not mirrored by design | VFS (data), LINK (page) | promo art: L! |
| I10 | VR modes and systems | O | `system/`, `mode/`, `story/`, `chronicle/`, `kizuna/` | ja + 8 | PAGE + VFS | `/wiki/quete`, `/wiki/niveau`, `/wiki/succes` (DB), no system/mode explainer pages | explainer pages (Story, Chronicle, Kizuna Station, Synergies) | VFS text (`help` 1,021, `system` 733 hash entries) + link | official prose: L! |
| I11 | VR guides and rules | O | `guide/{faq,cross-save,password}/`, `guideline/`, `tournament_guideline/`, `streamer-kit/` | ja + 8 | PAGE | none | FAQ and cross-save help; guideline text defines permitted reuse | LINK | guideline text is the governing licence-like text: L! (do not paraphrase) |
| I12 | VR competitions | O | `competition/`, `online-tournament/`, `heroes-showcase/`, `best-shot/`, `eve/`, `special/` | ja + 8 | PAGE | Achillea ranked (`packages/achillea-*`, `cron/tasks/achillea-*`) is RG's own; no mirror of official events | official tournament calendar/results | PROXY (dates, titles) | L! if results/images mirrored |
| I13 | VR community (INADAI, INAZUMA POST, questionnaires) | O | `inadai-<yymmdd>/`, `inazuma-post/`, `questionnaire/result*/` | ja (+ `inazuma-post/en/feedback/`) | PAGE | none | none | LINK | survey data: L! |
| I14 | VR localized mirror (8 langs) | O | `/victory-road/<lang>/...` | en fr it de es zh-tw zh-cn pt-br | PAGE | only en/fr topics; locale UI limited to fr/en/ja/es | full text in 6 languages | PROXY (hreflang map) | L! |
| I15 | Topics fragment | O | `/zukan/assets/inc/topics.html` | ja | PAGE | none | fragment empty today; watch only | PROXY (watch) | none |

### 4.2 Official: zukan.inazuma.jp

| ID | Page family / feature | Src | URL pattern | Langs | Data source | RG today | Gap | Target | Legal |
|---|---|---|---|---|---|---|---|---|---|
| Z1 | Character list (5,460) | O | `/[en/]chara_list/` | ja, en live | PAGE + VFS | `packages/inagle/src/zukan/{parser,direct,order}.ts`, `catalog.json`, cron `crawl` step `zukan` (order only, `crawlZukanOrder`), `zukan-audit.json` (5,463 audited, 12 issues), `/wiki/chara`, `/api/inazuma/character` | VFS-derived `inagle_characters` is the richer source; official page only needed for ordering and audit; `per_page=200` not exploited | VFS (data), PROXY (order/audit, 28 GETs) | metadata: no flag; portraits: see Z10 |
| Z2 | Character stats cards | O | `/[en/]chara_param/` | ja, en | PAGE + VFS | `/wiki/chara/[id]`, `inagle/src/stat-calculator.ts`, `wiki/chara-stats*.ts` | Lv50 official column used as a cross-check only | VFS | none |
| Z3 | Special moves (904) | O | `/[en/]skill/` | ja, en | PAGE + VFS | `/wiki/skill` (+`[id]`), table `inagle_skills`, cron `zukan:videos` writes `video_url`/`poster_url`/`thumbnail_url` | videos stay official-hosted URLs (reference); no checksum or availability monitor | VFS + LINK (video) | `.wmv` official videos: L! if rehosted; reference URLs only today |
| Z4 | Equipment (223) | O | `/[en/]item/equip/` | ja, en | PAGE + VFS | `/wiki/item` (+`[id]`), `inagle_items`, `wiki/item-category.ts` | none functional | VFS | none |
| Z5 | Fashion (uniforms) | O | `/[en/]item/fashion/` | ja, en | PAGE + VFS | `inagle_uniforms` (384 per `docs/gamedata-coverage.md`), no dedicated route | no uniform browser | VFS | textures: L! |
| Z6 | Emblems | O | `/[en/]item/emblem/` | ja, en | PAGE + VFS | `inagle_emblems`, `game/team-emblem-map.ts` | no dedicated route | VFS | art: L! |
| Z7 | 3D character viewer | O | `/chara_model_view/?q=` | n/a | VFS | `nie-model` service (`cdn.` asset origin), `iecode-dotnet` CDN, `/wiki/gallery` | no per-character 3D page on the site | VFS (nie-model, not vendored) | models are game assets: L! |
| Z8 | Team builder | O | `/my_team/` | ja, en | PAGE | `/wiki/tools/my-team` (`TeamBuilderLoader`), `/wiki/tools/random-team`, team codes (`game/team-code.ts`) | parity of constraints (30 slots, formation) unverified | VFS | none |
| Z9 | Formation tool | O | `/soccer_formation/` | ja, en | PAGE + VFS | `game/formations.ts`, `inagle_formations`; used inside my-team | no standalone page | VFS | none |
| Z10 | Portraits and assets | O | `dxi4wb638ujep.cloudfront.net/1/<x>/<y>/<z>/<id>.{png,webp}` | n/a | official CDN | `zukan/direct.ts` builds direct CloudFront URLs; `/api/inazuma/character` proxies allow-listed hosts; R2 mirror helper `lib/r2-assets.ts` | policy: reference or on-demand thumbnail only | PROXY (thumbnail on demand, size cap) | republishing art: L! |

### 4.3 Official: www.level5.co.jp

| ID | Page family / feature | Src | URL pattern | Langs | Data source | RG today | Gap | Target | Legal |
|---|---|---|---|---|---|---|---|---|---|
| L1 | Corporate home | O | `/`, `/top.html` | ja | PAGE | `level5.ts` (news step) | game pickup list not stored | PROXY | none |
| L2 | News | O | `/news/<yyyymmdd>[_NN]/` | ja | PAGE | `level5.ts` (crawl of `/news/`; index returns 403, slugs from links) | slug discovery relies on home and inazuma.jp links; press text not stored by design | PROXY (slug discovery) | text: L! |
| L3 | Studio blog | O | `/blog/`, `/blog/{en,zh-tw,zh-cn}/` | ja, en, zh-Hant, zh-Hans | PAGE | `level5-blog.ts` fetches all 4 | no site route; no ko | PROXY | text: L! |
| L4 | Inazuma dev blog | O | `/blog_inazuma/` | ja | PAGE | none | not crawled | PROXY | L! |
| L5 | Product catalogue | O | `/products/**` | ja | PAGE | `/wiki/jeux` partially from `series.ts` | 12 Inazuma product pages not ingested as structured data | PROXY (title, platform, date) | box art: L! |
| L6 | Events (TGS, LEVEL5 VISION) | O | `/tgs<year>/`, `/vision<year>/` | ja | PAGE | `docs/ie/ACTUALITES-IEVR-2026.md` (manual) | no event calendar | LINK + metadata | L! |
| L7 | CCC, ambassador, recruit | O | `/ccc/`, `/ambassador/`, `/recruit/` | ja | PAGE | none | off-topic | LINK | none |

### 4.4 Official: www.inazuma-cross.jp

| ID | Page family / feature | Src | URL pattern | Langs | Data source | RG today | Gap | Target | Legal |
|---|---|---|---|---|---|---|---|---|---|
| C1 | News + important + other posts (261) | O | `/information/{news,important,other_posts}/<id>/` | ja | API (RSS + sitemap) | `cross.ts` crawl, `apps/rg-website/data/inazuma-cross.json`, `/wiki/cross` | not driven by `lastmod`; only guide/cbt/campaign pages listed in `cross.ts` | PROXY (sitemap lastmod + feed `guid`) | `content:encoded` bodies: L! |
| C2 | Static pages (16) | O | `/terms/`, `/guidelines/`, `/guide/`, `/faq/`, `/parents/`, `/trade/`, ... | ja | PAGE | partial | terms and guidelines are governing text | LINK | L! (do not republish) |
| C3 | Shop | O | `store.inazuma-cross.jp` | ja | PAGE | none | none | LINK | none |

### 4.5 Community

| ID | Feature | Src | URL | Langs | Data source | RG today | Gap | Target | Legal |
|---|---|---|---|---|---|---|---|---|---|
| M1 | Fandom wikis (lore, series, VR) | C | inazuma-eleven.fandom.com (EN, FR), inazuma.fandom.com/es | en, fr, es | COMM (`api.php`) | `community-mods.ts` crawls the three `api.php` endpoints into an Orama index | no site search surface; attribution and licence handling not defined | PROXY (search, short excerpt, source link) | community licence terms (CC-BY-SA family) apply: flag |
| M2 | Player database and collection tracker | C | inadex.app, inazuma-eleven.fr/inadex | en | COMM | `/wiki/chara` (VFS-based, 5,460 rows) | RG lacks per-user collection tracking | none (build own on `@rosegriffon/auth`) | none |
| M3 | Mods, tools, undub, save editors | C | gamebanana.com, nexusmods.com, GitHub | multi | COMM (`apiv11`) | `/wiki/mods`, `community:mods` (14 GameBanana games, asset mirror to R2) | Nexus not reachable (403 challenge); `mirrorAssets: true` copies third-party files | PROXY (metadata) + LINK | mirrored mod assets: L! (third-party rights) |
| M4 | Datamines | C | Telmo26/ievr_dataminer, ievr_toolbox | multi | VFS-equivalent | inagle parsers (66 per `docs/gamedata-coverage.md`) read the same `common/text/<lang>` tree | none; use as a cross-check for non-decoded languages | VFS | none |
| M5 | JA move/tactic lists | C | w.atwiki.jp/inazumavr, game8.jp, gamecap.jp, wikiwiki.jp | ja | COMM | none for VR (atwiki older titles only) | acquisition locations are partly absent from the VFS-only view | LINK | scraping scope: flag |
| M6 | Competitive/tier lists | C | TheGamer, Dexerto, Operation Sports | en | COMM | `/wiki/tools/compare` | none | LINK | none |
| M7 | Community feed | C | reddit r/inazumaeleven, X `@inazuma_project`, `@LEVEL5_times` | multi | COMM | `reddit.ts`, `twitter.ts`, `x-accounts.ts`, `/feed` | none | PROXY | platform terms: flag |

## 5. Multilingual plan

### 5.1 What the game ships

`packages/inagle/src/core/data-loader.ts` `LOCALES` and `schemas/text.ts` `AllLocales`: nine locale folders under `data/common/text/<locale>/`: `ja en fr de es it pt zh_hans zh_hant`. Third-party tooling agrees: Mod Manager places text in `data/common/text/[language]/[file].cfg.bin`, `ievr_dataminer` writes one SQLite per language.

Decoded today: `packages/inagle/data/ievr-i18n.json` (source `ievr-vfs`, 32,229 hash entries, 23,457 distinct Japanese terms) carries only `ja en fr es` (32,227 / 32,223 / 32,223 / 32,223 non-empty). The `IevrTermEntry` schema already has `de it pt zh_hans zh_hant` fields, but they are empty; `GameTextLocale` in `game-text/shared.ts` is `fr en ja es`; `schemas/text.ts` states "Available locales: en, fr (with JSON exports)". The other five are UNVERIFIED in content (folder existence is declared, content not inspected here). Category weight (entries): chara 19,804, skill 2,784, item 2,880, menu 2,698, help 1,021, system 733, team 493, trophy 434.

`pt` is Portuguese in the VFS; the official sites publish `pt-br`. Treat `pt` as pt-BR only after a sample is checked against `/victory-road/pt-br/`.

### 5.2 What the official sites publish

| Surface | Languages |
|---|---|
| `inazuma.jp/victory-road/<lang>/` | en fr it de es zh-tw zh-cn pt-br (+ ja root) |
| `inazuma.jp/re/en/` | en |
| `zukan.inazuma.jp` | ja, en live; fr it de es zh-tw zh-cn pt-br advertised but 302 to ja |
| `level5.co.jp/blog/` | ja en zh-tw zh-cn |
| `level5.co.jp` main, `inazuma-cross.jp`, series microsites | ja only |
| Korean | nowhere (404 on all hosts) |

The nine official languages equal the nine VFS locales (ja, en, fr, de, es, it, pt-BR, zh-Hans, zh-Hant).

### 5.3 Mapping to `packages/i18n` (rg-i18n)

Current `DEFAULT_LOCALE_REGISTRY` (`packages/i18n/src/registry.ts`): default `fr`, plus `en`, `ja`, `es` (fallback `fr`). The language switcher filters to the same four; `apps/rg-website/src/lib/i18n/*-interface.json` exist for those four.

Proposal (no code written):

| Registry tag | VFS locale | Official site segment | Hreflang on official | Fallback chain |
|---|---|---|---|---|
| `fr` (default) | `fr` | `fr` | `fr` | none |
| `en` | `en` | `en` | `en` | `fr` |
| `ja` | `ja` | root | `ja` | `en`, `fr` |
| `es` | `es` | `es` | `es` | `en`, `fr` |
| `de` | `de` | `de` | `de` | `en`, `fr` |
| `it` | `it` | `it` | `it` | `en`, `fr` |
| `pt-BR` (alias `pt`) | `pt` | `pt-br` | `pt-BR` | `es`, `en`, `fr` |
| `zh-Hans` (aliases `zh-CN`, `zh-cmn-Hans`) | `zh_hans` | `zh-cn` | `zh-cmn-Hans` | `en`, `fr` |
| `zh-Hant` (aliases `zh-TW`, `zh-cmn-Hant`) | `zh_hant` | `zh-tw` | `zh-cmn-Hant` | `en`, `fr` |

Rules: UI strings through `rg-i18n` keys (English source, `fr` a catalogue); game text is not an interface string: it stays in `game-text` (`/api/text`) keyed by `hashId` and locale, with `pickLocale` extended to the nine-way order. One adapter module owns the three-way tag map (registry tag, VFS folder, official path segment) so no code hardcodes it. The `Accept-Language` resolver already in `accept-language.ts` handles `zh-CN`-style aliases once the aliases above are declared. Official-site text in a language the VFS lacks stays labelled with its source locale and is never machine translated silently (the existing `content_html_fr` machine translation of patch notes should carry an explicit label).

## 6. Proxy and cache design

Goal: real-time, scalable, polite access to official and community sources, serving normalised metadata and short excerpts, never a pass-through of official HTML or art.

Architecture, existing pieces first (`infra/services.toml`):

1. Edge: Cloudflare in front of the existing public nginx (`services.nginx`, vhost `cdn`; `infra/docker/cdn-edge/rosegriffon-cdn.conf` already shows `proxy_cache_path` + origin allow-list; `infra/docker/nginx/conf.d/00-base.conf` already declares zones `rg_next_static` and `rg_nie`). Add one zone `rg_official` (keys 64m, 2g) and a `location /v1/official/` in `rg-core.conf` with `proxy_cache_lock on`, `proxy_cache_use_stale error timeout updating http_500 http_502 http_503`, `proxy_cache_background_update on`. Cloudflare Cache Rules: respect origin `Cache-Control` from the core API, `stale-while-revalidate`.
2. Owner of fetch and normalisation: `services.core` (Elysia, vhost `api`), with `services.cron` (profile `cron-split`) as the scheduler of refresh jobs and `services.redis` as shared state. Browser-rendered fetch stays in `cron` (`horsServerless` tasks). `services.vfs` + `services.nie` own VFS-derived rows (never proxied from official). `services.cdn-sync` + R2 own the optional thumbnail store.
3. Public routes (proposal): `GET /v1/official/news?source=&lang=&before=`, `/v1/official/patchnotes?platform=&lang=`, `/v1/official/zukan/{characters,skills,items}` (order/audit only), `/v1/community/{wiki,mods}`; every object carries `source_url`, `fetched_at`, `source_lang`.
4. Freshness (no sitemaps except Cross):

   | Source | Driver | Poll | Edge TTL |
   |---|---|---|---|
   | inazuma.jp home TOPICS + `/victory-road/topics/` | new slug in TOPICS; origin `s-maxage=300` | 5 min, one conditional hash check | 300 s + SWR 3600 |
   | patch notes | `ver_X_Y_Z` slug appears | on new slug, then locale fan-out (8 langs) | 1 day, immutable after fetch |
   | level5 `/news/`, `/blog/` | slug discovery from home + inazuma.jp links; no sitemap | 15 min | 900 s |
   | inazuma-cross | `sitemap.xml` index `lastmod` (cheap) then child `lastmod`, plus `/feed/` `guid` | 15 min index, per-child on change | 300 s |
   | zukan | VR version bump (patch note) or weekly; `per_page=200` | 28 GETs per language (ja, en) | 7 days |
   | community (Fandom, GameBanana, Reddit) | native APIs, ETag where offered | 30 to 60 min | 1800 s |

   Where no validator exists, store `sha256(body)` in Redis (`fetch:<url>` -> `{hash, fetchedAt, status}`) and skip downstream writes when unchanged. Compute our own `ETag` from that hash so clients and Cloudflare revalidate cheaply.
5. Rate limits (outbound): one in-flight request per host, at least 1 s gap with jitter, hard cap 60 fetches per cron run, token bucket in Redis keyed `lock:<host>`, stop the run after two consecutive non-2xx, exponential backoff 30 min to 6 h on 429/5xx (same pattern as `feed-sync`), never `HEAD` on zukan (500), generic UA `rosegriffon/1.0 (+https://rosegriffon.fr)`. Inbound: `limit_req_zone` per IP on `/v1/official/` (for example 20 r/s burst 40) plus Cloudflare rate-limiting rule; anonymous responses cacheable, no per-user data.
6. Failure mode: stale-if-error up to 7 days; "failure is not empty" as in `feed-sync` (previous snapshot kept, task reports partial success).
7. Allow-list: the proxy fetches only hosts listed in a config file derived from this matrix (`www.inazuma.jp`, `zukan.inazuma.jp`, `www.level5.co.jp`, `www.inazuma-cross.jp`, community APIs); no URL parameter from clients (the current `/api/inazuma/character?url=` pattern is allow-list based and should be the model).

Gaps to close in the existing `crawl` pipeline before this works: make `news` (and the new patch-note task) serverless-safe, add the `lastmod` reader for Cross, extend `news.ts` from 3 to 9 languages, and move `official-topics.json` from a committed file to the cache store.

## 7. Top gaps by user value

1. Fresh official news and patch notes (I1, I6, I7): committed snapshot is 5 days old, crawl is manual; no sitemap means a polling proxy is the only route.
2. Game text in all nine VFS languages (section 5): five locales are undecoded; the UI offers four.
3. Patch notes per platform and version in all languages, as structured changes instead of stored HTML (I7).
4. Official VR localized content (I14, I10, I11): FAQ, cross-save, modes and systems, derived from VFS `help`/`system` text plus links.
5. Uniform and emblem catalogues (Z5, Z6): DB tables exist, no browsable pages.
6. Per-character 3D/portrait pages (Z7, Z10) through `nie-model` and on-demand thumbnails.
7. Official tournament and event calendar (I12, L6) and the beta-test archive (I8).
8. Level-5 studio and Inazuma dev blogs in four languages (L3, L4) surfaced on the site.
9. Inazuma Cross news by sitemap `lastmod` and full page set (C1).
10. Community wiki search with attribution (M1) and a collection tracker (M2).

## 8. Legal flags (for the Level-5 partner admin; no decision taken here)

- Existing agreement file `docs/legal/ACCORD_COMMERCIAL_RG-L5-VR-2026-001.pdf` was not read or interpreted here.
- Rows marked L! above republish or could republish: official article and patch-note text (I2, I6, I7, L2 to L4), game art and uniform/emblem textures (Z5, Z6, Z10), 3D models (Z7), official skill videos if rehosted (Z3, currently reference URLs), editions/prices (I5), survey results (I13), terms and guideline text (I11, C2).
- Already in the repo and worth the partner's review: table `patch_notes` stores full `content_html` and a machine-translated `content_html_fr`; `community:mods` runs with `mirrorAssets: true` (third-party files copied to R2); `/api/inazuma/character` proxies CloudFront images.
- `guideline/` (inazuma.jp) and `inazuma-cross.jp/guidelines/` forbid secondary use for commercial purposes and require the credit line quoted in the 2026-11-17 note; neither mentions data reuse. No terms were found for zukan. A missing `robots.txt` is not a licence.
- Community content: Fandom text and third-party mods carry their own terms; Nexus blocks bots (403 challenge), so only links.
- Open questions: contact Level-5 about zukan reuse; whether a polling proxy of public HTML at 5-minute cadence is acceptable; whether the official `/victory-road/<lang>/` text may be cached and shown for languages the site does not own; the `pt` vs `pt-BR` wording of the shipped text.

## 9. Verification log (this session)

Checked 2026-11-19: robots/sitemap on 5 hosts (statuses above); Cross sitemap counts parsed from the XML (188/69/4 posts, 16 pages, 2 misc); zukan locale redirects (curl `%{redirect_url}`); `per_page=200` (202 rows); `/victory-road/<lang>/` status for 9 codes; level5 blog languages; GitHub repo push dates via `gh api`. Not verified: `inadex.app` (TLS failure from this host), Fandom and Nexus (bot challenge), content of the five non-decoded VFS locales, Bold Revolution official URL, `store.inazuma-cross.jp` page set. Raw captures live only in the session scratchpad.
