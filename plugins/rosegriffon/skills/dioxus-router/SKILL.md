---
name: dioxus-router
description: "Use when working with the `dioxus-router` crate of dioxus: Cross-platform router for Dioxus apps."
---

# dioxus-router

- Version: 0.8.0-alpha.3 (workspace member of `third_party/dioxus`, vendored commit `525d4026c949`)
- Source: `third_party/dioxus/packages/router/` (manifest `third_party/dioxus/packages/router/Cargo.toml`)
- Upstream: https://github.com/DioxusLabs/dioxus/tree/main/packages/router
- License: MIT OR Apache-2.0
- In this monorepo the crate resolves through `[patch.crates-io]` in the root `Cargo.toml` to the vendored checkout, so there is exactly one version of it in the workspace.

## Purpose

Cross-platform router for Dioxus apps

## README (`third_party/dioxus/packages/router/README.md`)

# Dioxus Router

[Website](https://dioxuslabs.com) |
[Guides](https://dioxuslabs.com/learn/0.7/essentials/router/) |
[API Docs](https://docs.rs/dioxus-router/latest/dioxus_router) |
[Chat](https://discord.gg/XgGxMSkvUM)

## Overview

Dioxus Router is a first-party Router for all your Dioxus Apps. It provides an
interface similar to React Router, but takes advantage of types for more
expressiveness.

```rust, no_run
use dioxus::prelude::*;
use std::str::FromStr;

#[rustfmt::skip]
#[derive(Clone, Debug, PartialEq, Routable)]
enum Route {
    #[nest("/blog")]
        #[layout(Blog)]
            #[route("/")]
            BlogList {},

            #[route("/:blog_id")]
            BlogPost { blog_id: usize },
        #[end_layout]
    #[end_nest]
    #[route("/")]
    Index {},
}

#[component]
fn App() -> Element {
    rsx! {
        Router:: { }
    }
}

#[component]
fn Index() -> Element {
    rsx! {
        h1 { "Index" }
        Link {
            to: Route::BlogList {},
            "Go to the blog"
        }
    }
}

#[component]
fn Blog() -> Element {
    rsx! {
        h1 { "Blog" }
        Outlet:: { }
    }
}

#[component]
fn BlogList() -> Element {
    rsx! {
        h2 { "List of blog posts" }
        Link {
            to: Route::BlogPost { blog_id: 0 },
            "Blog post 1"
        }
        Link {
            to: Route::BlogPost { blog_id: 1 },
            "Blog post 2"

## Cargo features (from the manifest)

- `default` (enables 1)
- `wasm-split`
- `html` (enables 1)

## Depends on (same ecosystem)

`dioxus-history`, `dioxus-hooks`, `dioxus-html`, `dioxus-router-macro`, `dioxus-signals`, `dioxus-vdom`, `dioxus-vdom-macro`, `dx-env`

## Public items by file

- `third_party/dioxus/packages/router/src/components/child_router.rs`: struct ChildRouterProps, fn ChildRouter
- `third_party/dioxus/packages/router/src/components/default_errors.rs`: fn FailureExternalNavigation
- `third_party/dioxus/packages/router/src/components/history_buttons.rs`: struct HistoryButtonProps, fn GoBackButton, fn GoForwardButton
- `third_party/dioxus/packages/router/src/components/history_provider.rs`: fn HistoryProvider
- `third_party/dioxus/packages/router/src/components/link.rs`: struct LinkProps, fn Link
- `third_party/dioxus/packages/router/src/components/outlet.rs`: fn Outlet
- `third_party/dioxus/packages/router/src/components/router.rs`: struct OnInitialRouteResolved, struct RouterProps, fn Router
- `third_party/dioxus/packages/router/src/contexts/navigator.rs`: fn navigator, struct Navigator
- `third_party/dioxus/packages/router/src/contexts/outlet.rs`: struct OutletContext, fn use_outlet_context
- `third_party/dioxus/packages/router/src/contexts/router.rs`: struct ParseRouteError, fn root_router, struct ExternalNavigationFailure, struct RouterContext, struct GenericRouterContext
- `third_party/dioxus/packages/router/src/hooks/use_navigator.rs`: fn use_navigator
- `third_party/dioxus/packages/router/src/hooks/use_route.rs`: fn use_route
- `third_party/dioxus/packages/router/src/hooks/use_router.rs`: fn use_router, fn router, fn try_router
- `third_party/dioxus/packages/router/src/lib.rs`: mod navigation, mod routable, mod components, mod hooks, trait HasProps, mod exports
- `third_party/dioxus/packages/router/src/navigation.rs`: enum NavigationTarget, enum NavigationTargetParseError
- `third_party/dioxus/packages/router/src/routable.rs`: struct RouteParseError, trait FromQuery, trait FromQueryArgument, struct OptionMarker, trait ToQueryArgument, struct DisplayQueryArgument, trait FromHashFragment, trait FromRouteSegment, trait ToRouteSegments, trait FromRouteSegments, trait Routable, struct SiteMapSegment, enum SegmentType
- `third_party/dioxus/packages/router/src/router_cfg.rs`: struct RouterConfig

## How to use this skill

Read the files cited above at the vendored commit before writing code: the source in `third_party/` is the authority, not memory of older releases. Checks and builds of code using this crate run in the capped container on prd, never on the workstation.
