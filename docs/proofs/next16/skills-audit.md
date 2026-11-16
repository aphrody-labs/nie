# rg-website audit: Next.js 16.5.0-canary.52 (read-only)

Scope: `apps/rg-website` (sources under `src/`, so paths below are relative to `apps/rg-website/`). No build was run on this PC; findings come from source reading and the local docs in `node_modules/next/dist/docs` (01-app/02-guides: migrating-to-cache-components, instant-navigation, adopting-partial-prefetching).

Context7: the MCP server failed to connect (CONNECT_TIMEOUT 30 s; next-devtools and shadcn failed the same way). The local bundled docs were used instead, so no "recent 16.x changes" were verified through Context7.

Facts measured:
- `next.config.ts` has `reactCompiler: true`, no `cacheComponents`, no `partialPrefetching`, no `experimental.dynamicIO`.
- `package.json:38`: dev script is `next dev --webpack`. The dev-loop and the Turbopack-only tooling need Turbopack; production build is not pinned to webpack.
- 90 page files, 19 layouts (`src/app`). Route segment config: 111 `dynamic = "force-dynamic"`, 34 `dynamic = "force-static"`, about 50 `revalidate` exports, 7 `dynamicParams`, 43 `runtime = "nodejs"`.
- `typescript.ignoreBuildErrors: true` (`next.config.ts:~80`): type errors will not stop a build, which weakens any migration gate.

## 1. next-cache-components-adoption

Verdict: not adoptable as a flag flip. Enabling `cacheComponents: true` today fails the build on roughly 190 route files plus the root layout. Ranked by impact:

1. Incompatible route config (blocker, about 190 files). `cacheComponents` errors on any file exporting `dynamic`, `revalidate` or `fetchCache`. Inventory command: `grep -rnE "^export const (dynamic|revalidate|fetchCache)" src/app`. Examples: `src/app/page.tsx:27` (`force-static`), `src/app/wiki/page.tsx:40` and 110 more (`force-dynamic`), `src/app/wiki/skill/[id]/page.tsx:15` (`force-static` plus `dynamicParams`), the `revalidate = 3600/300/86400/1800/0` group. Change: delete each export; where `force-static` was used, move the data read into a `"use cache"` function with `cacheLife("hours")` (3600), `cacheLife("minutes")` or a custom profile (300, 1800, 86400); `revalidate = 0` and `force-dynamic` simply become uncached reads behind `<Suspense>`. `dynamicParams = true` is replaced by `generateStaticParams` plus Suspense-wrapped param reads. The `cache-components-instant-false` codemod does NOT remove these.
2. Root layout blocks every route (`src/app/layout.tsx:114`). `await Promise.all([cookies(), headers()])` at the top of `RootLayout` is a blocking request-time read; it is also why `src/app/page.tsx:25-27` needs `force-static`. Change: keep `<html>`/`<body>` outside the await; compute `lang`/`dir` in a Suspense-wrapped child, or (preferred) move locale to a path or rewrite segment (`/[locale]`, `proxy.ts` already exists at `src/proxy.ts`) so `params` carries it and `generateStaticParams` returns `fr|en|ja|es`. Consequence: pages become prerenderable and `force-static` at `page.tsx:27` is deleted. Fallback, if URL locale is rejected: render `<html lang="fr">` statically and set `lang`/`dir` from a small inline head script plus a Suspense-wrapped `<LocaleSync>`; this trades correct SSR `lang` for static shell, which is an SEO and a11y cost, so avoid it.
3. Second locale reader (`src/lib/i18n/server-locale.ts:14-15`). `getServerLocale()` calls `cookies()`/`headers()` inside try/catch; under Cache Components the catch no longer hides the prerender abort, and any Server Component calling it blocks. Change: accept `locale` as a parameter from `params`, or call it only inside Suspense children.
4. Other request reads reachable from pages: `src/lib/shop/cart.ts:51,75`, `src/lib/vroid/session.ts:133-169`, `src/lib/auth-helpers.ts:23`, `src/lib/api-auth.ts:26`, `src/app/settings/page.tsx:66`. Each page that reaches them needs a Suspense boundary at the reading child (`await params`/`searchParams` forwarded as promises, not awaited at the page top). `/api` routes are unaffected.
5. No existing `unstable_cache` or `"use cache"` (only a comment at `src/lib/api-client.ts:18`), so nothing to migrate there; `wikiService` reads (for example `src/app/wiki/chara/[id]/page.tsx:57,75,172-179,255`) are the first candidates for `"use cache"` with `cacheTag` for revalidation.
6. Sync IO: no `new Date()`/`Date.now()`/`Math.random()` in `app/layout.tsx` or `client-layout.tsx`; a repo-wide grep must still be run per build error, as component-level hits are unknown.
7. Client hook blocker: `usePathname` is imported in `src/app/client-layout.tsx:4` (root layout child) and 15+ filter/pagination components (`src/components/wiki/WikiPagination.tsx`, `GlobalSearch.tsx`, `src/components/news/CategoryChips.tsx`, and others). Under dynamic segments these raise `blocking-prerender-client-hook`; wrap `ClientLayout`'s pathname consumers (or the whole sidebar subtree) in `<Suspense>`.

Recommended path: incremental. Step A: remove the `dynamic`/`revalidate` exports and set `cacheComponents: true` together with the codemod's `instant = false` on all 109 segments, fix root layout (item 2), get `fr1` build green. Step B: remove opt-outs top-down, wiki first (most traffic, most static). Run builds on fr1 only.

## 2. next-partial-prefetching-adoption

Verdict: blocked until Cache Components is adopted (requires passing build). Nothing to flip today. Findings for when it is:

1. Explicit `prefetch` Links to audit (effective `prefetch={true}`): `src/components/wiki/CharacterSheet.tsx:222,872,885` and `src/components/wiki/WikiPagination.tsx:92` (bare `prefetch`). These currently warm the full legacy payload of the destination; each destination (wiki detail pages) needs an `instant()` preservation test before enabling the flag, then `export const prefetch = "partial"` per destination, then the global flag and the `remove-partial-prefetch` codemod.
2. Imperative prefetch: `src/components/wiki/GalleryCard.tsx:42-62` prefetches on `onMouseEnter`/`onFocus`; it has no Link insight and must be checked under `next start` after the flag (it may now only warm the App Shell).
3. `prefetch={false}` at `src/app/achillea/licence/licence-client.tsx:255`: likely unneeded afterwards, revisit separately.
4. Root layout (item 2 of section 1) decides the App Shell: if locale stays a cookie read, the shared shell is empty. The URL-segment locale is also what makes the shell cacheable per locale.
5. Tooling: Next dev with `--webpack` (`package.json:38`) loses `next-dev-loop`; switch the dev script to Turbopack (the config already has `turbopackFileSystemCacheForDev`).

## 3. next-bundle-optimizer (audit mode, no snapshot captured)

`next analyze` needs a build, which is forbidden on this PC; run on fr1: `bun --bun next analyze --output --snapshot rg-before` then `next analyze export --snapshot rg-before | gzip > before.jsonl.gz`. Source-level candidates, ranked (all unmeasured, heuristics only):

1. `src/app/client-layout.tsx` (410 lines, `"use client"` at the root layout): imports AppSidebar, Header, footer, navigation rail, language switcher, swipe detector, watermark and three context providers (lines 15-54) in one client module shipped to every route. Change: keep providers client-side but turn layout chrome (`PiedDePage`, `Header`, `MobileBottomNavigation`, watermark) into server components passed as children/slots, and `next/dynamic` the gesture and mobile-only widgets (`EdgeSwipeDetector`, `MobileWatermark`, `SidebarNavigationHandler`).
2. `recharts` loaded statically in `src/app/admin/_components/signups-chart.tsx:14` and `src/components/dashboard/database/element-chart.tsx:10`: admin/dashboard only; wrap the chart components with `next/dynamic` (`ssr: false`) at their page importers so they leave the shared graph.
3. `react-icons` is used in two page files (`src/app/achillea/guide/page.tsx:23`, `src/app/(sousdossiers)/projets/nostalgie/page.tsx:41`) with subpath imports, which is fine; `optimizePackageImports` already lists `react-icons`, `lucide-react`, `motion`, `recharts`. No config change needed; verify through the export that no barrel leaks remain (about 291 files import one of lucide, recharts, react-icons, date-fns, motion).
4. `language-provider.tsx:94` fetches `/locales/${language}.json` at runtime, which is good for size; check that `@rosegriffon/i18n` does not also bundle all catalogs (graph check on fr1).
5. `DeferredThirdParties.tsx` already defers third parties (positive); only 18 files use `next/dynamic`, against 373 `"use client"` files, so lazy boundaries around heavy interactive features (lightbox, `GalleryLightbox.tsx`, `DemoExperience.tsx`, comments) are the main untapped lever.
6. Client-dependency scan (server-only leaks): not run (needs the export). Priority candidates for the scan: `@rosegriffon/auth`, `@rosegriffon/db` reachable from `src/contexts/auth-context` and `src/lib/auth-roles`. Status: unverified.
7. Config: `webpack()` alias for `next/image` (`next.config.ts:~44`) is webpack-only and ignored by Turbopack; if production moves to Turbopack, the image-quota guard must move to `turbopack.resolveAlias`. `compress: false` is deliberate (nginx); `productionBrowserSourceMaps: false` is correct.

## 4. vercel-react-best-practices

Ranked by category impact:

1. async- (critical): `src/app/wiki/chara/[id]/page.tsx:172-179` runs three lookups sequentially (`getCharacterByBaseSlug`, then `getCharacterBySlug`, then `getCharacter`); the fallbacks are only needed on miss, so this is acceptable, but the whole page awaits `params` and then the full character before any streaming. Wrap the secondary sections (skills list at 229-261, cut-ins) in `<Suspense>` children that receive a promise, and start `getSkillCutins` in parallel with the skill lookups (`Promise.all` already used at 261).
2. server-cache-react (high): wiki pages call `wikiService.*` from `generateMetadata`, the page and `opengraph-image.tsx` for the same id; wrap the getters in `React.cache` (or `"use cache"` after adoption) to dedupe per request.
3. server-after-nonblocking: `after()` is not used anywhere in `src/app` or `src/lib`; analytics, view counters and audit-log writes in API routes and server actions should move into `after()`.
4. server-serialization: 373 client modules; `app-sidebar`, `ClientLayout` props and wiki detail components (`CharacterSheet.tsx`) receive large objects. Pass ids or the fields rendered only.
5. bundle-conditional / bundle-dynamic-imports: see section 3, items 1, 2 and 5.
6. rerender-: `reactCompiler: true` already handles memoization; remove manual `useMemo`/`useCallback` only after measuring, and make sure no component defines components inside components (compiler bails out silently). Check with `react-compiler-healthcheck` on fr1.
7. client-: `GalleryLightbox.tsx:123`, `SkillVideoActions.tsx:50`, `DemoExperience.tsx:265` use raw `fetch` in event handlers (acceptable); `DiscordSection.tsx:150` fetches on mount, and the app already ships `@tanstack/react-query` (optimized in config), so move it to the query client for dedup and caching.
8. rendering-: `app/layout.tsx:140` uses `dns-prefetch` for the Discord CDN; add `preconnect` for `cdn.rosegriffon.fr` (the asset origin, `next.config.ts` `assetPrefix`). `inlineCss: true` is a trade-off: it removes a blocking request but repeats CSS in every HTML response and defeats CSS caching across navigations; keep it only if Lighthouse still shows the gain after Partial Prefetching.

## Config observations (not skill output)

- `experimental` block at `next.config.ts:111`: `cpus`, `optimizePackageImports`, `serverActions`, `serverSourceMaps`, `turbopackFileSystemCacheForBuild/Dev`, `cssChunking`, `inlineCss`. None is `dynamicIO`/`useCache`, so nothing there is fatal; `cacheComponents` and `partialPrefetching` would be added at the top level, outside `experimental`.
- Several of these keys may now be stable or renamed in 16.5 canary; not verified without Context7. Check against `node_modules/next/dist/docs/01-app/03-api-reference/05-config/01-next-config-js/`.
- `typescript.ignoreBuildErrors: true` should be turned off before a migration, otherwise the build gate proves little.
