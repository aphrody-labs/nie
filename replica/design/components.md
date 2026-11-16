# Components (from recon, built in nie-ui-dioxus as UiNode helpers)

Token roles come from `tokens.<theme>.json`, which is generated from rg-ui. Every theme passes WCAG AA: 10 pairs each, 0 failures (`contrast.py`).

| Component | Variants | States | Tokens | a11y | Screens |
| --- | --- | --- | --- | --- | --- |
| Top bar | brand, nav links, language switcher | current page, focus | bg, text, text-muted, accent | `<nav>`, current link marked, 44 px targets | all |
| Menu drawer (mobile) | | open, closed | surface, text | focus trap, Escape closes | all |
| Hero title | display plus subtitle in the reading language | | text, text-muted | single h1 | S11-S19 |
| Title card | supported by nie or not | hover, focus | surface, border or accent, text | whole card is one link | S01 |
| Platform chip | one per platform | | surface, text-muted | text label, never colour alone | S01, S14 |
| Badge | nie status, NEW | | accent and on-accent, surface and text-muted | text | S01, S14 |
| List row with date | patch note, topic | NEW | surface, text, text-muted | `<time datetime>` | S01, S14 |
| School card | | hover, focus | surface, accent | link with the school name | S12 |
| Language toggle | en, fr, ja | current | accent, text-muted | `hreflang`, `lang` on each label | all |
| FAQ item | | collapsed, expanded | surface, text | `<details>` and `<summary>` | S18 |
| Footer nav | | | bg, text-muted | `<nav aria-label>` | all |

Visual assets are never taken from the source: no key art, logos or illustrations. Game images are decoded by nie from the player's own install.
