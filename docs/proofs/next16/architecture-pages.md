# Next.js 16.5.0-canary.52: Pages Router, architecture and config audit

Scope: docs `02-pages`, `03-architecture`, `04-community`, `index.md` under `node_modules/next/dist/docs`, against every Next.js app in the repo.
Coverage note: of the 02-pages tree, 54 files are Pages-specific, the rest are generated stubs ("DO NOT EDIT", `source: app/...`) that mirror the App Router docs. I verified the stub structure and read the config, turbopack, upgrade (`01-app/02-guides/upgrading/version-16.md`) and compiler pages that bear on this repo; I did not read the Pages-only data-fetching, routing and legacy-upgrade pages line by line because the repo has no Pages Router code (see below).

## Next.js apps found

`next` appears in five package.json files, but only one app builds Next:

- `apps/rg-website/package.json:122` (`next: catalog:`), the only app with `next.config.ts` and `src/app`.
- `packages/rg-app-shell`, `packages/rg-auth`, `packages/rg-ui`: peer or dev dependency on `next` for `next/link`, `next/headers`; no config.
- Root `package.json:129` pins the catalog to `16.5.0-canary.52`; `@next/bundle-analyzer`, `@next/env`, `@next/third-parties` are on the same version (lines 56-58).

## What differs in the Pages docs (summary)

- Pages-only surface: `pages/`, `_app`, `_document`, `_error`, `getStaticProps/getStaticPaths/getServerSideProps/getInitialProps`, `next/router`, `next/head`, `next/legacy/image`, API routes, `bundlePagesRouterDependencies`, Babel config, i18n routing, preview mode, `exportPathMap`.
- Pages-only config in the docs tree: `bundlePagesRouterDependencies`, `exportPathMap`, `generateEtags`, `onDemandEntries`, `useLightningcss`, `crossOrigin`, `webVitalsAttribution`.
- Shared with App Router: `turbopack`, `serverExternalPackages`, `transpilePackages`, `images`, `redirects/rewrites/headers`, `output`, `reactCompiler`, adapters, instrumentation, self-hosting.
- Architecture pages: Fast Refresh, Next.js Compiler (SWC: `removeConsole`, `reactRemoveProperties`, `styledComponents`, `emotion`, `relay`, `jest`, `modularizeImports`), supported browsers (Chrome/Edge/Firefox 111+, Safari 16.4+), accessibility (route announcer, built-in ESLint a11y plugin). Community pages cover contribution and the Rspack experimental adapter only.

## Pages Router leftovers

None found. There is no `pages/` or `src/pages`, no `_app`/`_document`, and a grep over `apps/rg-website/src` and `packages/*/src` finds no `next/router`, `getServerSideProps`, `getStaticProps`, `next/head`, `next/document`, `next/legacy/image`, `next/amp`, `images.domains`, or `experimental_ppr`. The site is pure App Router (`src/app`), with `src/proxy.ts` (already renamed from middleware) and `src/instrumentation.ts`.

One legacy API remains in App Router code, see finding 6.

## Findings, ranked by impact

### 1. Bundler split: dev and build run webpack, Turbopack config is dead weight (high)

- `apps/rg-website/package.json:38` `next dev --webpack`; `scripts/next-build.ts:46-49` appends `--webpack` unless a bundler flag is passed. Next 16 defaults to Turbopack; the guide ("Turbopack by default") lists `--webpack` as an explicit opt-out.
- `next.config.ts:31-33` sets `turbopack.root`, and `:143-144` sets `experimental.turbopackFileSystemCacheForBuild/ForDev`. Under webpack these do nothing, and the docs say both default to `true` anyway.
- `next.config.ts:34-50` is the only reason Turbopack is blocked: a `webpack()` hook with `resolve.extensionAlias` (`.js` to `.ts/.tsx`) and an alias `next/image$` to `src/lib/image-cdn.tsx`.
- Fix: port to Turbopack (`turbopack.resolveAlias: { "next/image": "./src/lib/image-cdn.tsx" }`, plus `turbopack.resolveExtensions`; the `.js` to `.ts` mapping needs a test since Turbopack resolves TS-extension imports itself), then drop `--webpack` from dev and build, and delete the `webpack()` hook. Until then remove lines 143-144 (no-ops) and keep `turbopack.root` only if Turbopack is run. Prove with `next build` (Turbopack) and a page screenshot.

### 2. `typescript.ignoreBuildErrors: true` (high)

- `next.config.ts:88`. Docs default is `false`; the flag lets production builds ship with type errors. A `typecheck` script exists (`package.json:50`) so the gate can move there.
- Fix: set `false` (or delete the block) and make CI run `typecheck`; fix residual errors first. With this on, `next build` no longer validates the Next 16 typed routes and async-API types.

### 3. `experimental.inlineCss: true` (medium)

- `next.config.ts:148`. Docs: `version: experimental`, replaces every `<link>` with `<style>`. On a site with a CDN `assetPrefix` (`:54`) it removes CSS caching across navigations and inflates every HTML response (note `compress: false` at `:52`, relying on nginx).
- Fix: keep only if Lighthouse LCP proof is archived in `docs/proofs/`; otherwise re-measure with `cssChunking` alone and drop it if the gain is under the HTML-size cost.

### 4. `experimental.cpus: 2` and `serverSourceMaps: true` (medium)

- `:112`, `:142`. `cpus: 2` caps build workers and slows builds on fr1; the memory-usage guide recommends `experimental.webpackMemoryOptimizations` and `serverSourceMaps: false` to cut memory. `serverSourceMaps` is only auto-enabled by `--enable-source-maps`-style CLI flags (`06-cli/next.md:307`); it costs memory in production builds.
- Fix: make `cpus` come from discovery (`os.availableParallelism()`) or drop it; set `serverSourceMaps` from an env flag, off by default, consistent with `productionBrowserSourceMaps: false` (`:92`).

### 5. `reactCompiler: true` with Cache Components not enabled (medium, optional)

- `:19`. Correct placement (top level since 16.0) and `babel-plugin-react-compiler` is present (`package.json:168`, root `:94` pinned `1.0.0`). The guide warns the compiler raises build time because it runs Babel. Webpack builds pay this twice.
- Fix: none required; verify the `babel-plugin-react-compiler` pin is the current stable release read live (`bun pm view`), and consider the `annotation` mode if build time on fr1 matters.

### 6. `legacyBehavior` on `next/link` (low)

- `packages/rg-app-shell/src/components/NavigationRail.tsx:59` `<Link ... passHref legacyBehavior>`. The 16.5 docs no longer document `legacyBehavior`; it is a Next 13 migration shim.
- Fix: put the className/props on `<Link>` directly (children are no longer required to be an anchor) and delete `passHref legacyBehavior`.

### 7. Image config versus 16 defaults (low, already compliant)

- `:672-723`: `qualities` explicit (`:679`, required since 16), `minimumCacheTTL` set to 31 days, `imageSizes` without 16 (matches the new default intent), `remotePatterns` used instead of `domains` (good), custom `loaderFile` gated by env.
- Residual: `dangerouslyAllowSVG: true` (`:721`) with CSP sandbox and `contentDispositionType: "attachment"` is the documented safe pairing; keep, but confirm no user-uploaded SVG is served from the optimizer path. The custom image loader (`cdn-loader.ts`) plus the webpack alias of `next/image` doubles the indirection: after finding 1, keep only the `loaderFile` route if the alias is not needed for the Hobby quota guard.

### 8. `serverExternalPackages` and `transpilePackages` overlap check (low)

- `:65-83` and `:90-107`. The docs state a package cannot be in both lists; none overlap now. `@sentry/nextjs` in `serverExternalPackages` (`:91`) is an unusual choice (Sentry's wrapper normally needs bundling); confirm with `sentry.*.config.ts` that instrumentation still runs on the standalone server.
- Fix: verify by build log; no change if tracing works.

### 9. `compress: false` and `poweredByHeader: false` (info)

- `:52-53`. Valid options. Compression is delegated to nginx per the self-hosting guide; confirm Cloudflare/nginx brotli is on for `text/html` since `inlineCss` raises HTML weight.

### 10. Upgrade-guide 16.x items already applied (no action)

- `middleware` renamed to `src/proxy.ts` with `export async function proxy` and a narrow matcher.
- Async request APIs: `await cookies()`/`await headers()` used (spot-checked `src/lib/shop/cart.ts`, `src/lib/i18n/server-locale.ts`).
- `next lint` absent from scripts (`oxlint` used, `package.json:43`); no ESLint config to migrate.
- `tsconfig.json:27-28` includes `.next/types` and `.next/dev/types`; `next-env.d.ts` imports `routes.d.ts` (typed routes active).
- No AMP, runtime config, `devIndicators` removed options, `unstable_` APIs or `experimental.ppr` flags in config.

### 11. Items to verify, not confirmed

- `data-scroll-behavior="smooth"` on `<html>`: none in `src/app/layout.tsx`; the 16 default no longer overrides scroll for smooth CSS. If `scroll-behavior: smooth` is wanted site-wide, add the attribute. `globals.css` shows only `overscroll-behavior` so likely nothing to do.
- Parallel routes need `default.tsx`: no `@slot` directories under `src/app`, so not applicable.
- Output: `output: "standalone"` (`:22`) with `outputFileTracingRoot` and `turbopack.root` on the same path; consistent with the monorepo guidance.

## Suggested order

1. Findings 2 and 1 (type gate, then Turbopack port with screenshot proof).
2. Findings 4 and 3 (build resources, inlineCss evidence).
3. Findings 6, 5, 8 (cleanups).
