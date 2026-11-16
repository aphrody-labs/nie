---
name: base-ui
description: Official Base UI (@base-ui/react v1.9+) reference and composition patterns for building accessible, unstyled React primitives. Use when working with @base-ui/react components, render props, data attributes, forms, or TypeScript types.
---

# Base UI (@base-ui/react)

Authoritative docs index: `https://base-ui.com/llms.txt` (each doc URL supports `.md`, e.g., `https://base-ui.com/react/handbook/composition.md`).

## Package Name
Always import from `@base-ui/react` (never the deprecated `@base-ui-components/react`).

## Core Rules
1. **Composition via `render` prop** (never `asChild`):
   - `<Menu.Trigger render={<MyButton size="md" />}>Open</Menu.Trigger>`
   - Custom components passed to `render` MUST forward `ref` and spread received props onto the underlying DOM element.
   - Multiple Base UI primitives can be composed by nesting `render` props (e.g. `Tooltip.Trigger` -> `Dialog.Trigger` -> `MyButton`).
   - Function form `render={(props, state) => <button {...props} />}` gives access to component state (`state.open`, `state.disabled`, etc.).
2. **Styling via `data-*` attributes & Tailwind v4**:
   - Base UI exposes state via `data-open`, `data-closed`, `data-disabled`, `data-highlighted`, `data-selected`, `data-checked`, `data-unchecked`, `data-starting-style`, `data-ending-style`.
   - Use CSS variables exposed by Base UI (`--anchor-width`, `--available-height`, `--transform-origin`) for popups/menus/selects.
3. **Animations**:
   - Use `data-starting-style` and `data-ending-style` for CSS enter/exit transitions without extra JS animation wrappers.
4. **Forms & Validation**:
   - Prefer `Field.Root`, `Field.Label`, `Field.Control`, `Field.Description`, `Field.Error` and `Form` for native + custom constraint validation.
5. **TypeScript**:
   - React 19 ref-as-prop works cleanly; use `React.ComponentProps<typeof Component.Part>` or Base UI's exported `*Props` types.
