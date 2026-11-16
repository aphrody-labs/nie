# Recon map: EA SPORTS FC Ultimate Team web app (reference for Achillea)

Read on 2026-11-16 in Claude in Chrome, in the user's own signed-in session, at human pace. Only navigation and reading: no purchase, listing, bid or pack opening. Screenshots are kept out of git, since they contain EA assets.

## Scope
- **App**: https://www.ea.com/ea-sports-fc/ultimate-team/web-app/, a single-page app with a left rail, a top status bar and a card-based hub.
- **Slice for Achillea**: squad building, objectives and challenges, card evolutions, club inventory, home hub. Achillea data comes from the Inazuma Eleven tables read by nie (characters, skills, teams). The economy uses Achillea points, never real money.

## Screens
| ID | Screen | How you get there | Key components | States seen |
| --- | --- | --- | --- | --- |
| U01 | Home hub | rail: Accueil | tile grid (season hub with countdown, challenges, promo carousel, evolutions), 2-step news modal on first load | modal, filled |
| U02 | Squads | rail: Équipes | active squad card (name, formation, rating, chemistry 33/33), squad management, team of the week | filled |
| U03 | Squad editor | U02, then active squad | pitch with 11 player cards placed by formation, chemistry markers per card, header (formation, star rating, chemistry, nation and league chips), side panel (formation mini-pitch with prev/next, formation select, set active, tactics, squad builder, rename, copy, clear), bench and reserves drawers | filled |
| U04 | Transfers | rail: Transferts | market search hero, transfer list (on sale, sold), transfer targets (won, outbid) | empty (0 items) |
| U05 | Store | rail: Boutique | packs (unseen dot), items, points | filled |
| U06 | Club | rail: Club | player items (66), quick-sell recovery, challenge storage, consumables, managers | filled |
| U07 | Challenges (SBC) | rail: DCÉ | filter tabs (all, favourites, players, upgrades, challenges, foundations), challenge cards (icon, title, description, progress bar, group reward, repeatable flag, expiry countdown, favourite star, reward preview card) | filled |
| U08 | Evolutions | rail: Évolutions | tabs (my evolutions, evolutions, training camp, rewards, cosmetics, roles, playstyle lab), evolution card (status chip Active, level 1 of 1, player card) | filled |
| U09 | Settings | rail: Paramètres | profile header (name, creation date, platform), language, sign out, play time, notifications, units, info, legal links | filled |

## Flows
- **A01 Build a squad**: U02, then U03, pick a formation, place cards from the bench or reserves, watch the chemistry. 2 clicks to edit.
- **A02 Complete a challenge**: U07, open a card, submit a squad that meets the requirements, collect the reward.
- **A03 Evolve a card**: U08, choose an evolution, complete its objectives, the card levels up.
- **A04 Open a pack**: U05, then packs, open, the items go to the club (U06).

## Components
Left rail (icon and label, unread dot), top status bar (coins, points, profile, club badge), hub tile, modal carousel (1/2, 2/2), player card (rating, position, nation, club, chemistry diamonds), pitch, formation mini-pitch, select, action list, filter tabs, challenge card with a progress bar, countdown, status chip, empty-count tiles.

## Inferred data model (Achillea names)
- **Card** (id, character_id, rating, position, rarity, attributes, evolution_level). Character data comes from nie cfg.bin. Confidence: high.
- **Squad** (id, owner, name, formation, slots[11], bench[], reserves[], chemistry). Confidence: high.
- **Formation** (code such as 4-2-1-3, slot positions). Confidence: high.
- **Challenge** (id, group, requirements, reward, repeatable, expires_at, progress). Confidence: high.
- **Evolution** (id, requirements, steps, upgrades, status). Confidence: medium.
- **ClubItem** (owner, card or consumable, tradeable). Confidence: high.
- **Wallet** (owner, coins, points). Earned in Achillea only, no real money. Confidence: high.

## Not cloned
- EA, FC, club, league and player names, likenesses, card art, logos, and the real-money store. Achillea uses Inazuma Eleven characters from the player's own install, its own card frames and our own tokens.
- The transfer market between users, which handles real economic value, is out of scope for now.

## Size
9 screens, 4 flows, 7 entities. Hard parts: the chemistry rules, challenge requirement validation, keeping the card economy fair. Size: M.

## Signed-out view (agent-browser, separate profile)
Login and "Find out more" buttons, the user agreement, privacy and collection notice links, and a "Rotate Device" heading for portrait phones. Everything else sits behind EA sign-in. That is why the screens above were read in the user's own session in Chrome.
