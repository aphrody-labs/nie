# Brand

- **Name**: Azalée, the existing Rose Griffon product name, with the series hub under `/series`. The game titles appear as names of the works, which is a factual reference.
- **Palette**: the Rose Griffon themes from rg-ui. Each theme has its own hue family and passes AA (see `design/`).
- **Logo**: the existing Rose Griffon and Azalée marks. The Level-5 marks are never used.
- **Voice**: direct, not curt; precise, not technical; warm, not promotional. Every string is written for us (`crates/azalee-app/locales`).
- **Sweep**: `sweep.py . --config replica/brand.json` must find no Level-5 asset, domain or brand string outside `replica/` and `data/series.json`, whose official links are intended.
