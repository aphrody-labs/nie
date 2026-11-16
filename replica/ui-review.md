# ui-ux-pro-max review of the Figma mockups (2026-11-16)

Query "game encyclopedia database dark" gives two recommendations:
- **product**: Wiki / Encyclopedia, Swiss style with flat design, a search-first landing, hierarchical navigation and a table-of-contents sidebar;
- **style**: Dark Mode (OLED), contrast 4.5:1 or better, visible focus, reduced motion.

| Frame | Finding | Fix |
| --- | --- | --- |
| Azalee series hub (6:2, 7:36) | no search; the source pattern is search-first | add a search field above the grid |
| All | 11-12 px badges and labels are below the 16 px base and 13 px minimum | badges at 13 px, body at 16 px |
| All | muted text at 60 % white on the dark surfaces | measure contrast against `onSurfaceVariant` from tokens.json, keep 4.5:1 or better |
| Mobile frames | bottom bar items are text only | 48 dp targets, 5 entries at most (respected) |
| nie studio (6:97) | the tree has no keyboard focus state | add a focus ring that uses the primary token |
