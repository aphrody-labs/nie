# Recon: inazuma.jp and zukan.inazuma.jp (2026-11-16)

Source: the HTML of https://www.inazuma.jp/ and https://zukan.inazuma.jp/, fetched on 2026-11-16. Japanese only (`lang="ja"`), no hreflang.

## inazuma.jp: series portal
- **Landmarks**: `header`, `section#visual` (key visual carousel), `section#infoArea` (news), `section#listArea` (titles), `footer`.
- **Title pages** (one per game): inazuma, inazuma2, inazuma3 (+ event, rendou), inazuma123, strikers, go, go2, gogalaxy, gostrikers, re, everyday, everyday-plus, victory-road.
- **Victory Road**, 162 of the home page links:
  - movie (32), patch notes per platform (ps-steam 27, xbox 26, switch 17);
  - beta-test (patchnote, tournament, vote), online-tournament, competition, heroes-showcase, character;
  - topics, special, questionnaire, guide/faq, best-shot, eve.
- **Other**: movie (topics, report, streaming), event, archive (stage, films), topics, anniversary pages (17th wallpaper, 18th), fes2018, inazuma-walker.
- **External**: level5.co.jp, inazuma-cross.jp, zukan.inazuma.jp, x.com, TV and publisher sites.

## zukan.inazuma.jp: official player encyclopaedia (Inagle)
Sections: chara_list, chara_param, skill, item, team_info, soccer_formation, my_team.

## Mapping to our data
- Characters, skills, items, teams and formations come from the cfg.bin tables imported by nie-cfgsql (71,103 files).
- Patch notes and news are editorial content with no game source; they go in Postgres and are edited from the admin.
- We never copy the official art, text or private endpoints. Visuals are our own tokens; images come from the player's own install through nie.
