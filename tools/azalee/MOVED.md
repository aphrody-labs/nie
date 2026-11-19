Moved from `rose-griffon/rosegriffon` (`packages/azalee`, `apps/rg-bot` azalee parts, 2026-11-19), as is. The wiki
library and CLI overlap `nie-wiki-vfs` and `nie-cli wiki`: dedupe against them before reviving. `tools/azalee-bot/` holds the
`/azalee` Discord command, its formatting, and the `/inagle`, wiki and azalee-bridge API modules of the old bot.
Imports of `@rosegriffon/*` are not rewired.
