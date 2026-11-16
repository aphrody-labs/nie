# Rose Griffon and Azalee Design System (`DESIGN.md`)

Canonical visual and token specification shared across Web (`apps/rg-website`), Native GUI (`nie.exe`, `crates/nie-ui`), Terminal (`ie` CLI / TUI), Mobile, and OpenAPI (`/api/design/tokens`).

## 1. Unified Theme Packs

Every selectable theme is defined as an OKLCH tonal seed in `packages/rg-ui/themes/<id>/theme.toml`, compiled into CSS custom properties (`--md-sys-color-*`) by `packages/rg-ui/scripts/generer-themes.ts`, projected to sRGB hex for Canvas/TUI/native consumers in `packages/rg-ui/src/tokens.ts`, and exported in `contract/stack.yml`.

| ID             | Label             | Mode  | OKLCH Seed              | Secondary / Overrides                                | Companion                       | Surface Role                        |
| -------------- | ----------------- | ----- | ----------------------- | ---------------------------------------------------- | ------------------------------- | ----------------------------------- |
| `roy`          | Roy               | light | `oklch(0.55 0.107 265)` | Navy & Gold DA                                       | Roy (`RG_Roy_salut.webp`)       | Default Association chrome          |
| `gaelle`       | Gaelle            | light | `oklch(0.55 0.164 34)`  | Vermilion & Gold DA                                  | Gaelle (`RG_Gaelle_salut.webp`) | Association alternate chrome        |
| `azalee-light` | Azalee clair      | light | `oklch(0.55 0.147 73)`  | Apricot & Crimson                                    | Reika (`RG_Reika_zukan.webp`)   | Wiki light mode                     |
| `azalee-dark`  | Azalee            | dark  | `oklch(0.55 0.126 77)`  | Apricot & Night amber                                | Reika (`RG_Reika_zukan.webp`)   | Default Wiki section scope          |
| `achillea`     | Achillea          | dark  | `oklch(0.55 0.267 297)` | Secondary `hue 84, chroma 0.164`, `neutral_chroma 0` | None                            | Default Esport section scope        |
| `inazuma`      | Inazuma Oficial   | light | `oklch(0.55 0.142 264)` | Secondary `hue 221, chroma 0.123`                    | Mark VR (`markvr.webp`)         | Official `inazuma.jp` portal tokens |
| `nie`          | NIE Pixel-Perfect | dark  | `oklch(0.55 0.096 258)` | Secondary `hue 94, chroma 0.128`                     | VR Badge (`vr.webp`)            | Native `nie.exe` desktop/TUI tokens |

## 2. Official `https://www.inazuma.jp/` Palette (`inazuma`)

Measured directly from upstream `shared.css`, `base.css`, `colorbox.css`, and `top_2608.css` (`--inazuma-*` tokens):

- **Navy Foundations**: `#082475` (`--inazuma-navy-dark`), `#102670` (`--inazuma-navy`), `#1b41b2` (`--inazuma-cobalt`), `#165ebf` (`--inazuma-azure`)
- **Electric Accents**: `#3ec4e9` (`--inazuma-cyan`), `#00a8ff` / `#34beed` / `#0092ef` (signal cyan highlights), `#ffe900` / `#ffd106` (`--inazuma-gold`), `#cf0000` / `#e60012` (`--inazuma-crimson`), `#ff418c` (event magenta)
- **Pitch / Field**: `#3e6817` (`--inazuma-turf`), `#7cff85` (`--inazuma-lime`)
- **Typography**: `Noto Sans JP` (400, 700, 900) primary for FR/EN/JA CJK parity, with `Poppins` headings, `Roboto` fallback, `Geist Mono` tabular stats
- **Shape & Radii**: `4px` small controls, `8px` cards/panels, `100px` / `9999px` pill CTAs

## 3. Native `nie.exe` Pixel-Perfect Tokens (`nie`)

Sourced from `crates/nie-ui/src/color.rs`, `surfaces.rs`, and `tokens.rs` (`--jeu-*` tokens):

- **Surfaces**: `#131420` (`--jeu-fond-abysse`), `#152d52` (`--jeu-fond-nuit`), `#224271` (`--jeu-fond-profond`), `#e0ecff` (`--jeu-surface-glace`)
- **Accents**: `#97c0fc` (`--jeu-accent-cyan`), `#5f9efc` (`--jeu-accent-turquoise`), `#e6c961` (`--jeu-accent-ambre`), `#c74955` (`--jeu-accent-brique`), `#0874ff` (`--screen-header-blue`), `#fcfaf8` (`--jeu-texte-vif`)
- **Geometry & Motion**: `--jeu-biseau: 14px` chamfered corners, `--jeu-rayon: 4px`, `--jeu-bordure: 2px`, `--jeu-duree-rapide: 120ms`

## 4. Inazuma Eleven Game Constants (Theme-Invariant)

- **Elements**: Fire (`oklch(0.6 0.16 29.3)`), Wind (`oklch(0.6 0.16 261)`), Forest (`oklch(0.6 0.16 140.4)`), Mountain (`oklch(0.6 0.139 52)`)
- **Positions**: GK `#fdd835`, DF `#1e88e5`, MF `#43a047`, FW `#e53935`

## 5. Quality & Anti-Slop Rules

1. **Spatial Rhythm**: Strictly 4dp/8dp grid steps (`0, 4, 8, 12, 16, 20, 24, 28, 32, 40, 48...`); no off-grid values (`5px`, `7px`, `14px` except explicit chamfer token `--jeu-biseau`, `18px`).
2. **Contrast Floor**: All `on-X / X` pairs must satisfy WCAG AA `>= 4.5:1` (`>= 7:1` at high contrast), verified automatically by `packages/rg-ui/test/color-engine.test.ts`.
3. **Zero Presentation Literals in UI TSX**: Components consume CSS custom properties and `@rosegriffon/ui` semantic utilities rather than raw hex constants.
4. **Zero Emojis**: Plain geometric SVGs, game sprites, and concise human prose across code, docs, CLI, TUI, and OpenAPI descriptions.
