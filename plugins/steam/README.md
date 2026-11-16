# steam plugin

Tools for the Steam ecosystem: an MCP server and a debugging agent. The Steam skills (steam-*, gaming-stack) come from the aphrody-skills plugin.

## Components

- `agents/steam-expert.md`: deep debugging agent (Proton, depots, auth).
- `servers/steam-mcp.ts`: MCP stdio server without npm dependencies started with `bun run`. Tools:
  `web_api_call`, `store_appdetails`, `resolve_steamid`, `local_apps`, `read_vdf`,
  `compat_tools`, `launch_env`, `app_paths`, `openid_login_url`, `openid_verify`, `steamdb_link`.
- Parsers (text and binary KeyValues, ACF, appinfo.vdf v39 to v41, libraries, Proton) are in `servers/steam.ts`, pure TypeScript.

## Environment

- `STEAM_WEB_API_KEY` (optional): Steam Web API key. The server sends it in the
  `x-webapi-key` header and never puts it in a URL or a tool result.
- `STEAM_PATH` (optional): Steam root override.

### Getting a Web API key

1. Sign in at https://steamcommunity.com/dev/apikey with a Steam account.
2. Enter the domain name the key is for and accept the Steam Web API Terms of Use
   (https://steamcommunity.com/dev/apiterms: 100,000 calls per day, keep the key private).
3. Store the key as `STEAM_WEB_API_KEY`: in the Windows user environment
   (`[Environment]::SetEnvironmentVariable("STEAM_WEB_API_KEY", "<key>", "User")`), in the
   untracked `.env` of the repo (the name is listed in `.env.example`), or in the shell
   profile on Linux. Restart Claude Code so `.mcp.json` picks it up.

### Without a key

Everything except the key-only Web API methods keeps working:

- `web_api_call` serves every method that `ISteamWebAPIUtil/GetSupportedAPIList` lists
  without a key (for example `ISteamUserStats/GetNumberOfCurrentPlayers`,
  `ISteamNews/GetNewsForApp`). A key-only method returns a clear "needs a Web API key"
  error instead of a raw HTTP 401 or 403; HTTP 429 is reported as rate limiting.
- `resolve_steamid` converts every numeric form offline and resolves vanity names through
  the public `steamcommunity.com/id/<name>/?xml=1` profile feed.
- `store_appdetails`, `openid_login_url`, `openid_verify`, `local_apps`, `read_vdf`,
  `app_paths` and `steamdb_link` never need a key.

## Other MCP clients

```json
{
  "mcpServers": {
    "steam": {
      "command": "bun",
      "args": ["run", "<path>/plugins/steam/servers/steam-mcp.ts"]
    }
  }
}
```

Requires Bun 1.4 or newer. No npm dependencies. License: Apache-2.0 (see the repository `LICENSE-APACHE`).
