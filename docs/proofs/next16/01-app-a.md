# Next.js 16.5.0-canary.52 docs, 01-app first half: audit of apps/rg-website

Scope note: the first half is 151 files (01-getting-started through 03-api-reference/04-functions/after.md), about 2.1 MB. It was not read line by line. Fully read: `02-guides/upgrading/version-16.md` (the authoritative change list), plus the headers and key sections of `instant.md`, `prefetch.md`, `proxy.md`, `reactCompiler.md`. The rest was grepped against the code. Findings below are only those verified in code.

## 1. New, changed, deprecated, removed in 16.x

New or changed
- Turbopack is the default for `next dev` and `next build`. A custom `webpack()` in next.config makes `next build` FAIL unless `--webpack` (opt out) or `--turbopack` (ignores webpack config) is passed. `turbopack` options moved top level (`turbopack: {}`, no longer `experimental.turbo`).
- Turbopack filesystem cache: `experimental.turbopackFileSystemCacheForDev` / `ForBuild`.
- Async Request APIs are mandatory: `cookies()`, `headers()`, `draftMode()`, `params`, `searchParams` are Promises, also for `opengraph-image`, `icon`, `apple-icon` (params and `id` async) and `sitemap` (`id` async).
- `middleware.ts` is renamed `proxy.ts`, exported function `proxy`, Node runtime only (`runtime` config not allowed in proxy).
- Caching APIs: `revalidateTag(tag, profile)` requires a second argument (cacheLife profile, e.g. `"max"`); new `updateTag` (Server Actions, read-your-writes) and `refresh`; `cacheLife`, `cacheTag` stable names; `unstable_cache` replaced by `use cache`.
- `cacheComponents` replaces `experimental.dynamicIO`, `experimental.useCache`, `experimental.ppr`. With it on, `export const dynamic`, `revalidate`, `fetchCache` are unsupported; use `use cache`, `cacheLife`, `Suspense`. New segment configs `instant`, `prefetch` (needs `cacheComponents`), `partialPrefetching` config, `ensureStatic`.
- `reactCompiler` is a stable top-level option (needs `babel-plugin-react-compiler`).
- React 19.2 (View Transitions, `useEffectEvent`, Activity). Built-in view transitions need no flag.
- Routing: layout dedup and incremental prefetch; new scroll handler default.
- `next/image` defaults changed: `minimumCacheTTL` 14400 s, `imageSizes` drops 16, `qualities` defaults to `[75]` (other `quality` values are rejected), local images with query strings need `images.localPatterns[].search`, local IP blocked (`dangerouslyAllowLocalIP`), `maximumRedirects` 3.
- Parallel route slots require an explicit `default.tsx`.
- `next dev` and `next build` can run concurrently (separate dirs).
- Build Adapters API (alpha), Modern Sass API, `next/root-params`.
- Node.js minimum 20.9, TypeScript 5.1.

Deprecated
- `next/legacy/image`, `images.domains` (use `remotePatterns`), `middleware` file name.

Removed
- AMP, `next lint` command (use ESLint/oxlint directly), `next.config` ESLint option, `serverRuntimeConfig` / `publicRuntimeConfig`, `devIndicators` sub-options, `experimental.dynamicIO`/`useCache`/`ppr`, `unstable_rootParams`, `experimental.viewTransition` (16.3), `experimental.appNewScrollHandler` (16.4).

## 2. Findings in apps/rg-website, ranked

### Correctness / build-breaking

1. Build path vs webpack-only aliasing. `next.config.ts:41-48` aliases `next/image` to `src/lib/image-cdn.tsx` inside `webpack()`. Under Turbopack (default) this alias is ignored, so all 121 `next/image` imports would hit the stock optimizer and lose the quota guard. Dev script (`package.json:38`) forces `--webpack`, and `scripts/next-build.ts:48-51` selects `--webpack` by default, so it is consistent today, but it pins the site to the opt-out path. Fix per docs ("Turbopack configuration location", "Resolve alias fallback"): `turbopack.resolveAlias: { "next/image": "./src/lib/image-cdn.tsx" }` (plus the `extensionAlias` equivalent via `turbopack.resolveExtensions`), then drop `webpack()` and `--webpack`.
2. `next.config.ts:87-88` `typescript.ignoreBuildErrors: true`. Type errors, including un-awaited async Request APIs, ship silently. Fix: turn it off and run `next typegen` + `tsc` in CI (docs: upgrading, "Migrating types for async Request-time APIs").
3. `src/instrumentation.ts:1-20`: `register()` body is commented out ("Edge Runtime errors"). `instrumentation.md` says guard with `process.env.NEXT_RUNTIME === "nodejs"` and dynamic-import Node code; the commented code uses `If (` (capital I, invalid). Fix: restore as an `if (process.env.NEXT_RUNTIME === "nodejs")` guard or delete the dead file.
4. `revalidateTag("articles", "max")` in `src/app/admin/chroniques/actions.ts:95,169,220,250` matches the 16 two-argument signature (good). Within Server Actions that need the user to see their own write immediately, docs prescribe `updateTag("articles")`; `revalidateTag(..., "max")` is stale-while-revalidate, so the admin may see old data after saving. Fix: use `updateTag` in these actions.
5. Async APIs: `cookies()`/`headers()` are awaited everywhere checked (`lib/api-auth.ts:26`, `lib/auth-helpers.ts:23`, `lib/vroid/session.ts:133-169`); no sync `params.` use found in `page.tsx`/`layout.tsx`/`opengraph-image.tsx`. No finding.

### Performance

6. Route segment config conflicts with a future move to `cacheComponents` (not enabled at `next.config.ts`): 113 files export `dynamic = "force-dynamic"`, many `force-static` plus `revalidate` (e.g. `app/page.tsx:27-28`, `app/wiki/tactic/[id]/page.tsx:2-4`, `app/wiki/skill/[id]/page.tsx:14`). These are valid now (PPR off), but `cacheComponents` rejects them. `force-dynamic` on list pages (`app/wiki/page.tsx:40`, `app/wiki/stade/page.tsx:1`, `app/wiki/succes/page.tsx:1`, `app/wiki/sons/page.tsx:26`) forgoes caching entirely. Fix: enable `cacheComponents`, replace with `"use cache"` + `cacheLife` + `cacheTag`, wrap request-time reads in `<Suspense>`; the project skills `next-cache-components-adoption` and `next-partial-prefetching-adoption` cover the steps.
7. Dynamic routes without `generateStaticParams`: only 5 files in `src/app` export it while dozens of `[id]` pages use `revalidate`/`force-static` with `dynamicParams = true`. Pages build on first hit (cold TTFB). Fix: return the hot ids (or an empty array, which makes the route ISR-capable at build) from `generateStaticParams`.
8. `export const runtime = "nodejs"` (e.g. `app/wiki/vroid/page.tsx:34`, `app/wiki/tools/my-team/opengraph-image.tsx:3`) is the default; redundant noise. `runtime` is also invalid with `cacheComponents`. Fix: remove.
9. `next.config.ts:140` `experimental.cpus: 2` and `inlineCss: true`: both still documented; `inlineCss` is fine, verify it stays under the page weight budget on large pages (docs warn it duplicates CSS per page).
10. `reactCompiler: true` (`next.config.ts:19`) is correct stable syntax; confirm `babel-plugin-react-compiler` is a dependency (build compile slows, docs note this). Consider `reactCompiler: { compilationMode: "annotation" }` if build time matters.

### Deprecation / hygiene

11. `images` config is correct for 16: `qualities` set (`next.config.ts` images block), `remotePatterns` used, no `images.domains`, no `next/legacy/image`, no `localPatterns` needed unless local images carry query strings. Docs: a custom `loaderFile` (`cdn-loader.ts`) must accept `quality` consistent with `qualities`; ok.
12. `proxy.ts` is correct (`proxy` export, narrow `matcher`, cookie-only optimistic check as `authentication.md` recommends). No `middleware.ts` remains.
13. No `default.tsx` and no `@slot` directories exist; the parallel route `default.js` requirement does not apply. No AMP, `serverRuntimeConfig`, `experimental.dynamicIO`/`ppr`/`viewTransition` flags found. `next lint` is not used (`oxlint` at `package.json:43`).
14. `lib/google-docs.ts:138` emits a raw `<img>` string into HTML; `images.md` prescribes `next/image` only for JSX, so this is acceptable for imported HTML, but lacks `loading="lazy"`/dimensions (CLS). Fix: add `loading="lazy" decoding="async"` and width/height when known.

## Not verified
Per-file reading of the remaining ~140 docs (API reference for `Link`, `Image`, `Form`, metadata files, `use cache` variants) was not done; the metadata-file and `generateMetadata` usage (39 callers) was not audited against `metadata`/`opengraph-image` rules beyond the async-params check.
