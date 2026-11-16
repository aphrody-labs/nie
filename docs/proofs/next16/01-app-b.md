# Next.js 16.5.0-canary.52: 01-app, second half, vs apps/rg-website

Scope: second half of `find . -type f | sort` under `node_modules/next/dist/docs/01-app/` (151 of 302 files, from `03-api-reference/04-functions/cacheLife.md` to the end: functions, next.config.js options, CLI, adapters, edge, turbopack, glossary). The set is about 974 KB; it was not read line by line. I read the files that bear on this app (functions, images, revalidateTag, route segment config, cacheComponents, turbopack, typescript, useTypeScriptCli, reactCompiler, inlineCss, CLI notes) and grepped the rest.

## 1. New, changed, deprecated or removed in 16.x

- `cacheComponents` is a top-level option since 16.0.0 (was `experimental.cacheComponents`). It replaces `ppr`, `useCache` and `dynamicIO` as one switch. Data fetching is dynamic by default; caching is opt-in via the `use cache` directive, `cacheLife`, `cacheTag`.
- With Cache Components on, the route segment configs `dynamic`, `dynamicParams`, `revalidate`, `fetchCache` are removed. `experimental_ppr` is removed (codemod exists). Without Cache Components they remain valid.
- `revalidateTag(tag)` single-argument form is deprecated (behaves like `{ expire: 0 }`). Use `revalidateTag(tag, "max")` (stale-while-revalidate) or `updateTag(tag)` in Server Actions (read-your-writes). `refresh()` is new, for refreshing uncached data from a Server Action.
- `unstable_noStore` deprecated since 15.0 in favour of `connection()`. `unstable_cache` is superseded by `use cache`.
- `next/image` in 16: `priority` deprecated, use `preload` (or `loading="eager"`); `qualities` default is `[75]` so every other `quality` value is rejected with 400; new `dangerouslyAllowLocalIP` and `maximumRedirects` configs.
- Turbopack is the default bundler for dev and build. Filesystem cache is `experimental.turbopackFileSystemCacheForDev` and `ForBuild` (both on by default); `turbopackGc`, `turbopackMemoryEviction`, `turbopackLazyDynamicImports`, `turbopackRustReactCompiler` and `turbopackLocalPostcssConfig` are new switches. Custom `webpack()` config is only honoured with `--webpack`; Turbopack uses `turbopack.resolveAlias`. The `edge-light` condition and the Edge runtime are documented as deprecated.
- `middleware.ts` is replaced by `proxy.ts` (Node runtime, `proxy` export); `proxyClientMaxBodySize` and `skipProxyUrlNormalize` are the renamed options.
- `params` and `searchParams` are async (Promise) in pages, layouts and `generateMetadata`; `cookies()`, `headers()`, `draftMode()` are async.
- `next lint` is gone; ESLint or oxlint run directly. `typescript.ignoreBuildErrors` still exists, and `useTypeScriptCli` documents the CLI type checker.
- New or notable: `forbidden()`/`unauthorized()` with `authInterrupts`, `use-offline`/`useOffline`, `catchError`, `prefetch`, `use-link-status`, `partialPrefetching`, `prefetchInlining`, `deploymentId`, `supportsImmutableAssets`, `outputHashSalt`, adapters API (`adapterPath`, `01-configuration` to `12-immutable-static-assets`), `next-root-params`, `image-optimizer-transform`, `agentRules`/`agentUpgrade`/`agentFeedback`, `instrumentationClientInject`, `htmlLimitedBots`.
- `experimental.inlineCss` is still marked experimental in its doc page.

## 2. Findings in apps/rg-website, by impact

1. **Image `priority` is deprecated (27 usages).** Examples: `src/components/wiki/GalleryCard.tsx:81`, `GalleryGrid.tsx:62`, `GalleryLightbox.tsx:218`, `shared/ImageWithSkeleton.tsx:38`, `shared/emotes/RgEmote.tsx:78`, `src/app/not-found.tsx:27`, `src/app/wiki/stade/[id]/page.tsx:102`, `wiki/cross/page.tsx:163,180`, `shop/[slug]/page.tsx:192`, `sondages/[pollId]/page.tsx:201`, `wiki/jeux/[slug]/GameDetailClient.tsx:95`. Fix: rename to `preload`; use `loading="eager"` where only lazy loading must be disabled. Note that `next.config.ts:51-54` aliases `next/image` to `src/lib/image-cdn.tsx` under webpack only, so the wrapper must forward `preload` too.

2. **`typescript.ignoreBuildErrors: true` at `next.config.ts:88`.** The docs describe it as allowing builds to finish with type errors and it also skips the CLI type check (`useTypeScriptCli.md`). Production can ship type errors silently. Fix: set `false` and fix the errors, or run `tsc --noEmit` as a required CI step.

3. **Root layout reads `cookies()` and `headers()` (`src/app/layout.tsx:114`).** This makes every route dynamic and conflicts with the static intent of `export const dynamic = "force-static"` plus `revalidate = 300` at `src/app/page.tsx:27-28`, `wiki/tactic/[id]/page.tsx:2-4`, `wiki/stade/[id]/page.tsx:1-2`, `wiki/succes/[id]/page.tsx:1-2`, `wiki/tools/random-team/page.tsx:1-2`. The docs prescribe, for 16: either keep the previous model (valid only while Cache Components is off) and move locale resolution out of the root layout (for example `rg-locale` handled in `proxy.ts` as a rewrite or a client read), or enable `cacheComponents` and wrap the locale read in `<Suspense>` with a `use cache` shell. Verify with a build report which routes are really static.

4. **Route segment configs will break the day `cacheComponents` is enabled.** About 40 pages export `dynamic`, `revalidate`, `dynamicParams`, `runtime` (for example `src/app/page.tsx:27`, `wiki/page.tsx:40`, `wiki/vroid/page.tsx:33-34`, `wiki/videos/page.tsx:45-46`, `wiki/textures/[[...path]]/page.tsx:28-29`, `sitemap.ts:11`, `wiki/sitemap.ts:45`). They are valid now (`cacheComponents` is not set in `next.config.ts`), so this is a migration risk, not a current bug. Fix path: `use cache` + `cacheLife("hours")` replaces `revalidate`; `force-dynamic` becomes reading `connection()` or request data under Suspense. `runtime = "nodejs"` is the default and can be dropped.

5. **`webpack()` hook and `next/image` alias at `next.config.ts:32-52` apply only to webpack builds.** `package.json:38` runs `next dev --webpack`, but `scripts/next-build.ts:48-51` passes `--webpack` only when requested. Under Turbopack the alias to `image-cdn.tsx` and the `extensionAlias` are ignored, so the image quota guard silently disappears. Fix: mirror both in `turbopack.resolveAlias` (the alias to `./src/lib/image-cdn.tsx`; extension resolution via `resolveExtensions`), or use `images.loader: "custom"` with `loaderFile` (already present at line 690 behind `CDN_LOADER`) as the single mechanism.

6. **`revalidateTag("articles", "max")` at `src/app/admin/chroniques/actions.ts:95,169,220,250` is correct for 16.** Since these are Server Actions that should show the editor's change immediately, the docs suggest `updateTag("articles")` for read-your-writes; keep `"max"` only where stale content is acceptable. `revalidatePath` calls (`lib/admin-garde.ts:73`, `lib/shop/cart.ts:194-222`) are fine.

7. **`experimental` flags worth re-checking** (`next.config.ts:130-155`): `turbopackFileSystemCacheForDev/ForBuild: true` are the defaults, so redundant; `serverSourceMaps` is documented only in `06-cli/next.md:307`, as a side effect of `--enable-source-maps`; `inlineCss` and `cssChunking` are valid. `cpus: 2` has no doc page in this half. Fix: drop redundant lines, keep the rest.

8. **`compress: false` (`next.config.ts:92`)** is valid when nginx compresses; the docs only require a reverse proxy to do it. Confirm nginx gzip or brotli is on for `/_next/static` and HTML, otherwise payloads go out uncompressed.

9. **Edge runtime.** No `runtime = "edge"` found; `sentry.edge.config.ts` exists and Edge is documented as deprecated for Next code, so remove it if no route uses it.

10. **Clean items (no action):** `src/proxy.ts` uses the 16 `proxy` export and a narrow `matcher`; `cookies()`/`headers()` are awaited everywhere; no `next/legacy/image`, `next/head`, `next/router`, `images.domains` or `onLoadingComplete`; `images.qualities` at `next.config.ts:679` already lists every used quality; `reactCompiler: true` is top-level as documented (needs `babel-plugin-react-compiler` installed); `remotePatterns` used instead of `domains`; no Pages Router API, no `unstable_noStore`, no single-argument `revalidateTag`; `app/actions/zukan.ts:8` takes a plain object named `searchParams`, not Next route props.

## Limits

The second half was skimmed by grep and targeted reads, not read in full; adapters docs (`01-app/.../adapters/*`) and the glossary were not read. Findings 3 and 5 depend on build behaviour and should be confirmed with a build output (`next build` route table) on the build host.
