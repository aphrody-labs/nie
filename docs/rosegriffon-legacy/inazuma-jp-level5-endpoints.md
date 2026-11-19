# Inazuma Eleven / Level-5 public endpoints: live discovery

Discovery date: 2026-11-16 23:46 to 2026-11-17 UTC (server `Date` headers). Method: about 130 serial GETs with `curl -sS -A "rosegriffon-research/1.0 (+https://rosegriffon.fr)" -L --max-time 20`, DNS over Cloudflare DoH, certificate transparency via crt.sh. No logins, no form posts, no media downloaded. Everything below was observed in that session unless marked **UNVERIFIED**.

Target use: headless/serverless community hub (wiki + REST API) on Cloudflare Workers + D1/KV + R2, metadata and text extracts only, no bulk republishing of media.

## 1. Summary table of hosts

| Host | Purpose | HTTP | Server / CDN hints | robots.txt | Sitemap | Feed |
|---|---|---|---|---|---|---|
| `www.inazuma.jp` | Official Inazuma Eleven series portal + Victory Road site (news, patch notes, characters, systems) | 200 | `Server: openresty`, `Cache-Status: oke`, CNAME `www.inazuma.jp.id200812.cas.iijcdn.jp` (IIJ CDN), A 202.32.8.150 / 210.149.92.76 | 404 | 404 (`/sitemap.xml`, `/sitemap_index.xml`) | none found (`/feed`, `/rss.xml`, `/atom.xml`, `/feed.xml`, `/index.xml` all 404) |
| `inazuma.jp` (apex) | Redirect | 301 to `https://www.inazuma.jp/` | `Server: Apache` | 301 then 404 | n/a | n/a |
| `zukan.inazuma.jp` | "Inagle", official player encyclopedia for Victory Road (characters, stats, special moves, items) | 200 | `Server: nginx/1.30.6`, CNAME `nieweb-lb-1235124860.ap-northeast-1.elb.amazonaws.com` (AWS ALB, Tokyo); assets on CloudFront `dxi4wb638ujep.cloudfront.net` | 404 | none found | none found |
| `s-zukan.inazuma.jp`, `d-zukan.inazuma.jp` | Staging/dev aliases (names from certificate SAN) | 302 to `https://zukan.inazuma.jp:443/` | `Server: awselb/2.0` | redirect | n/a | n/a |
| `www.inazuma-cross.jp` | Official site, mobile game "Inazuma Eleven Cross" (Aiming + Level-5), WordPress | 200 | `server: nginx`, `via: 1.1 google`, `EO-LOG-UUID` / `EO-Cache-Status` (Tencent EdgeOne), CNAME `www.inazuma-cross.jp.eo.dnse5.com` | 200, allows all except `/wp-admin/` | 200, sitemap index (3 children) | 200 RSS 2.0 `/feed/` |
| `website-origin.inazuma-cross.jp` | Origin named in that site's robots.txt `Sitemap:` lines | `/sitemap.xml` 403 | `server: nginx`, `via: 1.1 google` | not requested | 403 | n/a |
| `www.level5.co.jp` | Level-5 corporate site: product pages, news, blog | 200 | `Server: openresty`, `Cache-Status: oke`, CNAME `www.level5.co.jp.id200810.cas.iijcdn.jp` | 404 | 404 (`/sitemap.xml`) | none (`/rss.xml` 404) |
| `inazuma-world.com` | Legacy fan-world teaser page (linked from inazuma.jp), `www` 302 to apex | 200 | `Server: nginx`, A 112.78.213.169 | 404 | not probed | not probed |
| `inazuma-ac.jp` | Arcade title page | 301 to `https://www.takaratomy-arts.co.jp/specials/inazuma-ac/` | `Server: Apache` | 404 at target | not probed | not probed |
| `inazuma-eleven.com` | Domain unrelated to the series (parked) | 200 | `Server: Parking/1.0`, A 64.190.63.222 | not probed | n/a | n/a |

Certificate transparency (crt.sh, 2026-11-17):
- `%.inazuma.jp`: inazuma.jp, www, zukan, d-zukan, s-zukan, `*.inazuma.jp`.
- `%.level5.co.jp`: level5.co.jp, `*.level5.co.jp`, adfs, gp1, oaw, ps1, secure. These last five were **not requested** (look like internal/auth hosts; out of scope).
- `%.inazuma-eleven.com`: inazuma-eleven.com and wildcard only; the homepage is a domain-parking page, not official.

Outbound links seen on `www.inazuma.jp` (not inventoried): `www.tv-tokyo.co.jp/anime/inazuma2018/`, `s.mxtv.jp/anime/inazuma/`, `promo.kadokawa.co.jp/mfkids/inazuma/dvd/`, `corocoro.tv`, `www.artemis-holyroad.jp`, `avex.jp/frame/`, `x.com/inazuma_project`, `store.inazuma-cross.jp`, app stores (`apps.apple.com`, `play.google.com`, out of scope). Third-party scripts: Google Tag Manager `GTM-KX7QTFN` (inazuma.jp, zukan), `GTM-NT76CFBR` (inazuma-cross), `GTM-TCB894P` (level5), Rakuten tag `ias.r10s.jp`.

## 2. Per-host sections

### 2.1 www.inazuma.jp

Static, server-rendered HTML (jQuery 3.6, no XHR/fetch data calls found in the home page; no `/api/`, no `.json` references). No JSON-LD. No hreflang on the portal home; hreflang exists inside the Victory Road section.

Headers (identical on all 9 sampled pages): `Cache-Control: max-age=300, s-maxage=300`, `Cache-Status: oke; fwd=miss`, `Strict-Transport-Security: max-age=315360000`, `Vary: Accept-Encoding`. No `ETag`, no `Last-Modified`. Conditional GET therefore not usable; use content hashing.

Metadata (home): `<title>イナズマイレブンシリーズ</title>`, `og:type=website`, `og:site_name=イナズマイレブンシリーズ`, `og:image=https://www.inazuma.jp/assets/img/shared/ogp.jpg`, `twitter:site=@inazuma_project`, description "レベルファイブが贈る超次元サッカーゲーム「イナズマイレブン」シリーズ公式サイト！..." Victory Road section: `og:title=イナズマイレブン 英雄たちのヴィクトリーロード`, `og:image=https://www.inazuma.jp/victory-road/assets/img/shared/ogp.jpg`.

News list (the only list surface): the home page embeds a TOPICS block, 287 `<dl>` entries in the HTML, newest dated 2026.11.11 at fetch time:

```html
<li><dl><dt>2026.11.11</dt>
<dd>『イナズマイレブン 英雄たちのヴィクトリーロード』<a href="/victory-road/patchnote/switch/ver_7_2_2/" target="_blank"><br class="spNone" />ver.7.2.2配信のお知らせ（Nintendo Switch 2／Nintendo Switch）</a></dd></dl></li>
```

No pagination parameters; there is no list endpoint beyond the home block and the section pages. 48 distinct `/topics/<slug>` slugs are linked from the home page.

Path templates (all linked from the home page; 8 section pages fetched and returned 200):
- `/victory-road/topics/<yymmdd>[_NN]/` news article (e.g. `261020`, `251113_01`).
- `/victory-road/patchnote/{switch|ps-steam|xbox}/ver_<maj>_<min>_<patch>/` patch notes (e.g. `ver_7_2_2`); beta: `/victory-road/beta-test/patchnote/ver_1_1_4/` to `ver_1_2_1/` (also `_ps`, `_switch` suffixes); `/victory-road/patchnote/switch/251114/`.
- `/topics/<yymmdd>/` portal-level news (`190926`, `230822`, `240328`); `/movie/topics/<yymmdd>/`.
- Series directories: `/inazuma/`, `/inazuma2/`, `/inazuma3/`, `/inazuma123/`, `/inazuma3_rendou/`, `/go/`, `/go2/`, `/gogalaxy/`, `/gostrikers/`, `/strikers/`, `/strikers_extreme` (image only), `/re/`, `/ff/index.html`, `/everyday/`, `/everyday-plus/`, `/inazuma-walker/`, `/victory-road/`, `/archive/{butai,eiga}/NN/`, `/fes2018/`, `/event/`, `/movie/`.
- Victory Road sections: `/victory-road/{character,character/nagumohara-select,system,mode,product,guideline,tournament_guideline,streamer-kit,competition,online-tournament,eve,best-shot,heroes-showcase,inazuma-research,inazuma-post,questionnaire,sale,special,guide/faq,guide/cross-save,guide/password,topics/inazuma}/`.
- Languages: `/victory-road/{en,fr,it,de,es,zh-tw,zh-cn,pt-br}/` (hreflang block lines 40-48; `/victory-road/en/` 200, 55,713 B, `<html lang="en">`, title "INAZUMA ELEVEN: Victory Road"). No ko page found. The top-level `/en/` is 404.
- Assets: same-origin `/assets/img/...` and `/victory-road/assets/img/...` (no separate CDN host). Not counted or downloaded.

Copyright: footer `<li>&copy;LEVEL5 Inc.</li>`; also the notice "ゲーム中に登場する必殺技や特訓は、架空のものです。危険ですので、絶対に真似をしないでください。"

### 2.2 zukan.inazuma.jp (Inagle)

The richest structured surface. Server-rendered HTML tables and lists, GET-only, no auth, no JSON API found in the pages fetched (**UNVERIFIED** whether the JS files expose XHR endpoints; the JS bundles were not downloaded). Note: `HEAD` on a list URL returned 500 (`HTTP/1.1 500 INTERNAL SERVER ERROR`, 265 B); use GET. No `ETag`/`Last-Modified`/`Cache-Control` headers on GET responses (only `Date`, `Content-Type`, `Server`).

Languages (hreflang on the page, `http://` URLs): ja `/`, `/en/`, `/fr/`, `/it/`, `/de/`, `/es/`, `/zh-tw/`, `/zh-cn/`, `/pt-br/`. Verified `/en/chara_list/` returns 200 with the same 5460 result count and English names ("Mark Evans", "Axel Blaze"). Other locales beyond en not individually fetched. No ko.

Pages and counts (result counts shown on page):
- `/chara_list/` 5,460 characters, 50 rows per page by default, 110 pages (`?page=110` link seen). Params in the form: `name_search`, `per_page` (50/100/150/200), `page`, `rc`, `gender_filter`, `attr_filter`, `pos_filter`, `grade_filter`, `team_filter`, `version_filter`, `category_filter`.
- `/chara_param/` 5,459 results; detail cards with stats.
- `/skill/` 904 special moves, 19 pages at 50.
- `/item/equip/` 223 items, 5 pages at 50; siblings `/item/fashion/`, `/item/emblem/` (linked, not fetched).
- `/chara_model_view/?q=` 3D character view (linked, not fetched).
- `/my_team/` (team builder, 30-slot UI), `/soccer_formation/` (formation tool). `/team_info/` 302 to `/`.

Opaque query parameter `q` (decoded, verified): `base64url( bytes_of(JSON) XOR 0xFF )`. Decoded examples: `{"filter_chara_id_str":["c01000010"]}` (chara_param), `{"character_id":["c01000010"]}` (chara_model_view), `{"category_filter":[30]}` (item/equip). Character ID shape: `c` + 8 digits (`c01000010` is Mark Evans). Internal asset id shape: `<a>/<b>/<c>/<11-char id>` (`k/d/w/dwho-wi8ruk`). The `q` obfuscation is trivial but a deliberate form obfuscation (form class `obfuscated-form`); treat reliance on it as a stability risk and prefer the plain GET params where they exist.

`/chara_list/` row fields (first row, verbatim values): `data-chara-id="k/d/w/dwho-wi8ruk"`, name `円堂 守`, reading `えんどう まもる`, nickname `円堂`, appearance flags for 9 titles in fixed order (IE1, IE2, IE3, GO1, GO2, GO3, Ares, Orion, Victory Road; `○`/`×`), gender `男`, attribute `山`, position `GK`, role `選手`, era `中学生`, grade `二年生`, team `雷門中<br />イナズマジャパン`, description text.

`/chara_param/` card adds: acquisition routes (e.g. "プレイヤーズユニバース: とっくんノート座, ハンド座, ダイヤモンド座", "クロニクル対戦ルート"), and seven stats (kick, control, technique, pressure, physical, agility, intelligence) with a Lv50 column.

`/skill/` entry: name (ruby), a description line, and media: `data-movie-url=https://dxi4wb638ujep.cloudfront.net/1/k/t/q/tqgunvnu5vk.wmv` plus poster jpg and webp. Locked entries render `？？？` with a placeholder. Video files must not be ingested; only store text.

`/item/equip/` entry: name with ruby, image (`alt="[雷門/らいもん]シューズ"`), no stats in the list view.

Image/asset conventions (paths only, nothing downloaded):
- Character/item/skill art: `https://dxi4wb638ujep.cloudfront.net/1/<x>/<y>/<z>/<id>.{png,webp,jpg,wmv}`, with `onerror` fallback to `/1/prd/assets/4/img/shared/icn_secret_character.png`, `icn_secret_item.png`, `thum_secret_special-move.png`.
- UI assets: `https://dxi4wb638ujep.cloudfront.net/1/prd/assets/4/{css,js,img}/...` (the numeric `4` is an asset release number).
- Title icons: `icn_ie1 icn_ie2 icn_ie3 icn_go1 icn_go2 icn_go3 icn_ars icn_ori icn_vic`.

Metadata: `<title>イナズマイレブン公式選手図鑑 Inagle</title>`, description "『イナズマイレブン 英雄たちのヴィクトリーロード』のキャラクター情報などが検索可能！", `og:image=https://dxi4wb638ujep.cloudfront.net/1/prd/assets/4/img/shared/ogp.jpg`. No JSON-LD.

### 2.3 www.inazuma-cross.jp (WordPress)

- `robots.txt` (full text): `User-agent: *` / `Disallow: /wp-admin/` / `Allow: /wp-admin/admin-ajax.php` / `Sitemap: https://website-origin.inazuma-cross.jp/sitemap.xml` / `Sitemap: https://website-origin.inazuma-cross.jp/sitemap.html`. The origin host answers 403; the public equivalent `https://www.inazuma-cross.jp/sitemap.xml` answers 200.
- `/sitemap.xml` is a sitemap index (Yoast-style children): `sitemap-misc.xml` (2 URLs), `post-sitemap.xml` (247 URLs: 176 `/information/news/<id>/`, 67 `/information/important/<id>/`, 4 `/information/other...`), `page-sitemap.xml` (16 URLs: `/terms/`, `/privacy/`, `/guidelines/`, `/guide/`, `/faq/`, `/parents/`, `/trade/`, `/tutorial-gacha/`, `/samuraiblue2026/`, `/victory-road-2026/`, ...). Header `Cache-Control: no-cache`; `lastmod` per child (post-sitemap 2026-11-13T02:16:46+00:00).
- `/feed/`: RSS 2.0 with `content:encoded`, 10 items, `lastBuildDate: Fri, 13 Nov 2026 02:16:46 +0000`; item shape: `<title>`, `<link>https://www.inazuma-cross.jp/information/news/36539/`, `<pubDate>`, `<category>` (e.g. お知らせ, ガチャ), `<guid isPermaLink="false">https://www.inazuma-cross.jp/?p=36539`, `<description>` and full `<content:encoded>` HTML. No ETag/Last-Modified; `Cache-Control: no-cache`.
- `/wp-json/` returns `401 {"code":"rest_disabled","message":"The REST API on this site has been disabled."}`. `/wp-json/wp/v2/posts` redirects (301) and is not usable.
- Media: same origin `/wp-content/uploads/<hash>.{jpg,webp,png}`; theme assets under `/wp-content/themes/officialsite/dist/`; YouTube thumbnails `img.youtube.com/vi/<id>/maxresdefault.jpg`.

### 2.4 www.level5.co.jp

Legacy table-based HTML, same IIJ/openresty front (`Cache-Control: max-age=300, s-maxage=300`, `Cache-Status: oke; hit`). No robots, no sitemap, no feed. Useful surfaces:
- `/products/index.html` (200, 106 KB) lists Inazuma product pages: `/products/inazuma/{inazumaeleven,inazumaeleven2,inazumaeleven3,inazuma123,strikers,strikers2012x,go,go2,gogalaxy,gostrikers,everyday-plus,victory-road}.html`, plus links out to `https://www.inazuma-cross.jp/` and `https://www.inazuma.jp/re/`. `/products/inazuma/victory-road.html` returns 200 (title "イナズマイレブン 英雄たちのヴィクトリーロード｜株式会社レベルファイブ"). The directory `/products/inazuma/` and `/news/` return 403 (Apache-style listing blocked).
- News articles: `/news/<yyyymmdd>[_NN]/` (200, e.g. `/news/20260609/`, title tag generic "お知らせ｜株式会社レベルファイブ", body contains the press text). No index endpoint; slugs are discovered from links (home, inazuma.jp topics).
- Copyright: `©LEVEL5 Inc. All Right Reserved.` (footer image alt).

### 2.5 Minor hosts

- `inazuma-world.com`: 555-byte static page, `Last-Modified: Thu, 29 Oct 2026 03:47:09 GMT`, `ETag: "22b-65ef288bc4a86"`; no data value.
- `inazuma-ac.jp` redirects to Takara Tomy Arts; out of scope.
- `inazuma-eleven.com`: parked, not official; do not ingest.

## 3. Endpoint catalogue

All methods GET, auth none, unless noted. "Stability" is my judgement from observed structure (low = likely stable, high = likely to change).

| # | URL / template | Format | Sample (first lines) | Freshness signals | Stability risk | Ingestion |
|---|---|---|---|---|---|---|
| 1 | `https://www.inazuma.jp/` (TOPICS block) | HTML | `<dt>2026.11.11</dt><dd>...ver.7.2.2配信のお知らせ` | Cache-Control 300 s, no validators | Medium (markup in `topics_2608.css` era, name changes per redesign) | Cron every 15 min, hash body, parse `li > dl` |
| 2 | `https://www.inazuma.jp/victory-road/topics/<yymmdd>[_NN]/` | HTML | n/a (not sampled in detail) | same | Low | Fetch once per new slug found in #1, store title/date/text extract |
| 3 | `https://www.inazuma.jp/victory-road/patchnote/{switch,ps-steam,xbox}/ver_X_Y_Z/` | HTML | linked from #1 | same | Low | Same as #2, structured by platform and version |
| 4 | `https://www.inazuma.jp/topics/<yymmdd>/`, `/movie/topics/<yymmdd>/` | HTML | linked from #1 | same | Low | Same as #2 |
| 5 | `https://www.inazuma.jp/victory-road/{en,fr,it,de,es,zh-tw,zh-cn,pt-br}/...` | HTML | `<html lang="en">` | same | Low | Mirror text extract per locale for static pages only |
| 6 | `https://www.inazuma.jp/victory-road/guideline/` | HTML | "動画・静止画投稿に関するガイドライン" | same | Low | Store hash + short quote; see legal section |
| 7 | `https://zukan.inazuma.jp/[<locale>/]chara_list/?page=N&per_page=P&...filters` | HTML table | `data-chara-id="k/d/w/dwho-wi8ruk" data-chara-name="円堂 守"` | none | Medium (IDs stable by design; markup may change) | Weekly crawl, 5,460 rows |
| 8 | `https://zukan.inazuma.jp/[<locale>/]chara_param/?q=<obf>` or `?page=N` | HTML cards | `{"filter_chara_id_str":["c01000010"]}` decoded | none | Medium | Weekly, paginated list for stats |
| 9 | `https://zukan.inazuma.jp/[<locale>/]skill/?page=N` | HTML list | `<ruby>ザ・ウォール</ruby>` + description | none | Medium | Weekly, 904 entries, text only (skip .wmv) |
| 10 | `https://zukan.inazuma.jp/[<locale>/]item/equip/?page=N` (+ `/item/fashion/`, `/item/emblem/`) | HTML list | `alt="[雷門/らいもん]シューズ"` | none | Medium | Weekly, 223 equip items |
| 11 | `https://zukan.inazuma.jp/chara_model_view/?q=<obf>` | HTML | `{"character_id":["c01000010"]}` decoded | none | High (3D viewer) | Do not ingest, link only |
| 12 | `https://dxi4wb638ujep.cloudfront.net/1/<x>/<y>/<z>/<id>.{png,webp,jpg}` | image | n/a | CloudFront (headers not sampled) | Medium | Reference by URL; cache-on-demand in R2 with source URL + sha256 |
| 13 | `https://www.inazuma-cross.jp/feed/` | RSS 2.0 | `<title>【予告】［天空をつらぬく足］桜咲 丈二 ピックアップガチャ開催！</title>` | `lastBuildDate`, `Cache-Control: no-cache`, no ETag | Low (WordPress core) | Cron every 15 min, upsert on `guid` |
| 14 | `https://www.inazuma-cross.jp/sitemap.xml` -> `post-sitemap.xml`, `page-sitemap.xml`, `sitemap-misc.xml` | XML | `<sitemap><loc>.../post-sitemap.xml</loc><lastmod>2026-11-13T02:16:46+00:00` | `lastmod` | Low | Daily; use `lastmod` to decide which `/information/news/<id>/` to fetch |
| 15 | `https://www.inazuma-cross.jp/information/{news,important,other}/<id>/` | HTML | 176 news + 67 important | no-cache | Low | Backfill once (247 URLs), then incremental via #13/#14 |
| 16 | `https://www.inazuma-cross.jp/wp-json/` | JSON | `{"code":"rest_disabled",...}` (401) | n/a | n/a | Not available; do not retry |
| 17 | `https://www.level5.co.jp/products/index.html`, `/products/inazuma/*.html` | HTML | 12 Inazuma product pages | Cache-Control 300 s | Low | Daily, product catalogue (titles, series) |
| 18 | `https://www.level5.co.jp/news/<yyyymmdd>[_NN]/` | HTML | `2026年6月9日 ... イナズマイレブン クロス` | same | Low | Discover slugs from #1 outbound links; fetch on new slug |
| 19 | `https://www.inazuma.jp/assets/img/shared/ogp.jpg`, `/victory-road/assets/img/...`, `/wp-content/uploads/...` | image | n/a | n/a | Low | Reference only |
| 20 | DNS (DoH `https://cloudflare-dns.com/dns-query?name=<host>&type=A`, `accept: application/dns-json`) | JSON | CNAME `www.inazuma.jp.id200812.cas.iijcdn.jp` | n/a | n/a | One-off, not recurring |

Count: 10 hosts in the summary table, 20 catalogue rows (including the DNS and image-reference rows; 16 are content-bearing fetch surfaces).

## 4. Ingestion plan (Worker + D1/KV + R2)

General rules:
- One `User-Agent` of the form `rosegriffon/1.0 (+https://rosegriffon.fr)`; one request at a time per host, minimum 1 s gap; hard cap per cron run (for example 60 fetches).
- Validators: none of the Japanese hosts returned `ETag` or `Last-Modified` on GET (only `inazuma-world.com` did). Instead store `sha256(body)` per URL in KV (`fetch:<url>` -> `{hash, fetchedAt, status}`) and skip D1 writes when unchanged. Still send `If-None-Match`/`If-Modified-Since` if a validator ever appears; it costs nothing.
- Respect `Cache-Control: max-age=300` on inazuma.jp and level5: never poll those hosts more often than every 5 min; every 15 to 60 min is plenty.
- Store extracted fields and text only in D1; keep the source URL and `fetched_at` on every row. Do not store raw HTML except a hash.

Jobs:
1. News (every 15 min): fetch `https://www.inazuma.jp/` and `https://www.inazuma-cross.jp/feed/`. Parse TOPICS `li > dl` (date, lead-in label, link text, absolute URL) and RSS items (`guid`, title, link, pubDate, categories, description). Upsert into D1 `news(source, source_id, url, title, published_at, lang, categories, summary, hash)`. For new inazuma.jp slugs (`/victory-road/topics/...`, `/patchnote/...`) fetch the page once, extract title and visible text, store in D1 `news_body`.
2. inazuma-cross backfill (once, then daily): sitemap index, then `post-sitemap.xml` 247 URLs, using `lastmod` to skip unchanged ones.
3. Zukan (weekly, serial): per locale (start with `ja`, `en`, `fr`; others later) fetch `chara_list` pages (110 requests at 50/page; **UNVERIFIED** whether `per_page=200` is honoured by GET, which would cut it to 28), `chara_param` pages, `skill` (19 pages at 50), `item/equip` (5 pages at 50), plus `fashion` and `emblem`. Parse rows into D1: `character(id=c########, asset_id, name, reading, nickname, gender, attribute, position, role, era, grade, team, description, appears_ie1..vic)`, `character_stats(character_id, kick, control, technique, pressure, physical, agility, intelligence, level)`, `skill(name, description, locale)`, `item(name, reading, category)`. Translations key on `character_id` (and `asset_id`), with locale-specific name rows. Total weekly cost: roughly 140 to 330 GETs per locale; at 1 s spacing that is a few minutes of Worker time when split into Queue batches or Durable Object alarms (Workers have per-invocation subrequest limits, so use Queues/Cron chunks of 40).
4. Level-5 product catalogue (daily): `/products/index.html` and the 12 Inazuma product pages; extract title, series, links.
5. Images: never bulk-copy. Record the official URL (`cloudfront.net/1/<x>/<y>/<z>/<id>.webp` or same-origin path) in D1. Serve through the API as `source_url`. If a thumbnail is needed on pages, cache on demand: on first request, fetch once, store in R2 at `img/<sha256>.<ext>` with object metadata `source-url`, `fetched-at`, `etag/hash`, and cap size (for example 256 KB per object, character portraits only). Do not fetch `.wmv`/video files, 3D model views, wallpapers, or press kits.
6. Rate/cost guard: KV key `lock:<host>` with TTL; backoff on 429/5xx (zukan returns 500 for HEAD); stop the job on two consecutive non-2xx.

REST shape suggestion (read API served from D1): `/v1/characters`, `/v1/characters/{c01000010}`, `/v1/skills`, `/v1/items`, `/v1/news?source=&lang=&before=`, `/v1/patchnotes?platform=` with `source_url` on every object.

## 5. Legal and robots notes

robots.txt per host (checked 2026-11-17):
- `www.inazuma.jp`: 404 (no rules).
- `zukan.inazuma.jp`: 404 (no rules).
- `www.level5.co.jp`: 404 (no rules).
- `www.inazuma-cross.jp`: 200; `Disallow: /wp-admin/`, `Allow: /wp-admin/admin-ajax.php`, two `Sitemap:` lines (origin host; public `www` equivalent works).
- `inazuma-world.com`, `inazuma-ac.jp` targets: 404.
A missing robots.txt is not a licence; the notes below are the operative constraints.

Key text found (Japanese, quoted exactly):
- `https://www.inazuma.jp/victory-road/guideline/` ("動画・静止画投稿に関するガイドライン"): "本ゲームの動画・静止画等および本素材を二次利用（本ゲームの動画・静止画等または本素材を利用して制作した動画・イラスト・商品等の販売）またはその他の営利目的に利用する行為" is listed as a prohibited act; ownership: "本ゲームおよび本素材の著作権・著作隣接権その他の知的財産権は、当社または当社が指定する第三者に帰属します。" Required credit line for posts: "©2025 LEVEL5 Inc." The guideline governs gameplay video/still posts by individuals and states non-commercial, personal use; it also says corporate use must go through the Level-5 contact form. It does not mention text/data scraping.
- `https://www.inazuma-cross.jp/guidelines/`: same structure, includes "(8) 本動画を二次利用（本動画を利用して制作した動画・イラスト・商品等の販売）またはその他の営利目的に利用する行為" and "本ゲームの著作権・著作隣接権その他の知的財産権は、当社又は当社が指定する第三者に帰属します。" `https://www.inazuma-cross.jp/terms/` is the app terms of service (user-generated content licence to the operator; IP belongs to the operator).
- Copyright notices: `©LEVEL5 Inc.` (inazuma.jp), `©LEVEL5 Inc. All Right Reserved.` (level5.co.jp).
- No explicit data-reuse or API terms were found for zukan; its footer/about modal text was not extracted (**UNVERIFIED**).

Practical stance for the hub: factual metadata (names, IDs, dates, titles, short descriptions as attributed excerpts) with a link back to `source_url`; no mirroring of art, videos, wallpapers, music, or full article bodies; show the "©LEVEL5 Inc." attribution; non-commercial fan-project positioning; provide a takedown contact; and honour any future robots.txt/terms change. This is a technical record, not legal advice.

## 6. Open questions

1. Zukan XHR/JSON: do the bundled JS files (`shared.js`, `tile.js`) call a JSON endpoint? Not inspected (JS not downloaded); the HTML tables are sufficient for now.
2. Does `per_page=200` work on GET for `chara_list`/`skill`/`item`? Form offers it; only HEAD was tried (returned 500).
3. Locale coverage: `/en/chara_list/` verified; confirm `fr`, `it`, `de`, `es`, `zh-tw`, `zh-cn`, `pt-br` return the same 5,460 rows. Korean is not offered by either host.
4. Does the zukan contain pre-Victory-Road characters only as appearance flags, or complete data for older titles (IE1 to Orion)? Appearance flags cover 9 titles.
5. Terms for the zukan itself (no robots.txt, no visible reuse statement found); consider asking Level-5 via the corporate contact form.
6. Are `s-zukan`/`d-zukan` ever publicly serving different content? Both currently redirect to production.
7. `inazuma.jp` homepage markup includes a CSS file named `top_2608.css`; dated asset names suggest periodic redesigns, so selectors should be tested by a fixture-based parser test.
8. Older series sites (`/inazuma/`, `/go/`, ... ) were linked but not fetched; their structure and data depth are unknown.
9. `www.level5.co.jp/blog/` and `/ambassador/` were linked from inazuma.jp but not probed.
10. `ias.r10s.jp` (Rakuten tag) and Google Tag Manager are trackers; not data sources.
