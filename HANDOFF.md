# ingenuity-bot — Session Handoff

> Paste this whole file (or point the new session at it) to catch up instantly.
> Last updated: 2026-06-24. Owner: Jordan Boyd (boyd.jordan.t@gmail.com).

## What this project is
A desktop tool (later also a website) for **Path of Exile 2** (the ARPG by Grinding Gear Games / GGG).
First feature: a **Tablet Mod Valuator** — it reads live trade listings for the endgame
Atlas "Precursor Tablets," then estimates the average in-game value of each tablet
*modifier* so players know what each mod is worth. **No arbitrage framing** — the user
dropped that word; this is a market-value/insight tool, not an arbitrage tool.

App name: **ingenuity-bot** (do not use "arbitrage" anywhere in the name, UI, or repo).

## Hard constraints (non-negotiable — these came from the user)
1. **GGG Terms of Service compliance is the top priority.** GGG is historically aggressive
   about third-party tools. Everything below is a ToS requirement, not a nicety.
2. **User-Agent on every request:** `OAuth ingenuity-bot/0.1.0 (contact: boyd.jordan.t@gmail.com)`
3. **Honor rate-limit headers** (`X-Rate-Limit-*`, `Retry-After`) with a real header-driven
   rate limiter in the Rust backend. NOT fixed sleeps. Known trade-API limits (IP-based):
   - Search: 5/12s, 15/62s, 30/302s
   - Fetch: 12/6s, 16/14s
   - Exchange: 5/17s, 10/92s, 30/302s
4. **Cache aggressively.** Refresh only on explicit user action.
5. **No automation of in-game actions.** Read-only market data only. No auto-trade, no
   auto-whisper, nothing that touches the game client or files.
6. **Show the GGG disclaimer in the UI:** "This product isn't affiliated with or endorsed
   by Grinding Gear Games in any way."
7. **Privacy — no persistent personal data.** Don't write account name, character names,
   or stash contents to disk. OAuth tokens go in the OS keychain (tauri-plugin-stronghold),
   never plaintext. Anything user-specific is in-memory only. No telemetry/analytics/crash
   reporting with user data. Market/price cache CAN persist (not personal). Document in
   docs/privacy.md.
8. **Login = OAuth only, never passwords.** Users never type their pathofexile.com password
   into our app.

## Chosen stack (already decided — don't re-litigate)
- **Tauri 2** desktop shell (small native Windows binary, same web UI reusable on a website later).
- **Frontend:** React + TypeScript + Tailwind.
- **Backend:** Rust (rate limiter + trade API client live here).
- User is on **Windows 11**. If Tauri prereqs are missing (Rust toolchain, MS Build Tools,
  WebView2), detect and walk the user through install — don't assume present.
- **User is non-technical.** Drive end-to-end. Ask about product/UX in plain English;
  make all implementation calls yourself without asking.

## Research findings (already done — trust these)
### Trade data source: the trade-site internal API (NOT scraping HTML, NOT the documented API)
- Two-step flow on `www.pathofexile.com`:
  1. `POST /api/trade2/search/{league}` with filter JSON → returns a list of item IDs.
  2. `GET  /api/trade2/fetch/{comma,separated,ids}` → returns full listings (mods + prices + seller).
- This is unofficial-but-tolerated; every major community tool uses it (Exiled Exchange 2, Sidekick).
- **Trade search works anonymously** (rate-limited by IP). Logging in raises the limit.
  => The tablet valuator needs **NO login**. Make login optional; only prompt when a future
  feature (e.g. stash valuation) needs it.
- Current PoE2 leagues seen via `/api/trade2/data/leagues`: "Runes of Aldur", "HC Runes of
  Aldur", "Standard", "Hardcore". Default to the current softcore temp league ("Runes of Aldur").
- Stat IDs for mods come from `/api/trade2/data/stats` (e.g. `explicit.stat_2390685262` =
  "#% increased Quantity of Items found in Map"). Item taxonomy from `/api/trade2/data/items`.

### Tablets (the first feature's subject)
- 8 tablet types: Breach, Expedition, Delirium, Ritual, Irradiated, Overseer, Abyss, Temple.
- Each is **Magic rarity**: exactly 1 prefix + 1 suffix.
- **Prefix pool is shared across all types** (~7 mods: item quantity, rarity, pack size,
  magic monsters, rare monsters, gold, experience — all "in your Maps").
- **Suffix pool is type-specific** (8–9 each, buffing that type's mechanic). ~70–80 mods total.
- Plan: per tablet type, pull listings, parse each listing's prefix+suffix+price, run a
  **linear regression of price vs. mod presence/magnitude**. Each coefficient = average
  value of that mod. Report coefficient + sample size + confidence interval so the user sees
  which estimates are trustworthy vs. noisy.
- Reference for item/mod data: https://poe2db.tw/ (HTML only — no public API/JSON; ingest by
  parsing or hand-curating the mod list. poe2db has no data export).

### Currency
- PoE2 economy tiers: **Exalted Orb** = base everyday unit; **Divine Orb** = high value
  (~118 Exalted each, fluctuates); **Chaos Orb** = small change.
- **Normalize all prices to Exalted Orbs** using live exchange rates from the trade API.

### OAuth (for the optional login, later)
- GGG OAuth = **email request only**, no self-service portal. Email `oauth@grindinggear.com`.
- Requests are low-priority, can take weeks, and **LLM-generated/low-effort requests are
  auto-rejected** — the email must be human-written by Jordan. A draft was prepared (see below).
- Flow: Authorization Code + **PKCE** (SHA256). Public client. Access token 10h / refresh 7d.
- Minimum scope for any login: `account:profile` (display name only). Tablet valuator needs ZERO.
- **Action item for Jordan:** send the OAuth registration email (draft is in the prior session;
  regenerate if needed). Until a client_id arrives, ship a "Login with Path of Exile" button
  that's present but disabled/"coming soon." Architect auth so going live is a one-config change.

## Build plan (phases — check in with user after each)
1. ✅ Research (done — this document).
2. Scaffold Tauri 2 + React + TS + Tailwind at the repo root. "Hello world" window launching
   on Windows. Commit. Add docs/tos-compliance.md and docs/privacy.md.
3. Rate-limited Rust trade-API client (compliant User-Agent + header-driven limiter + caching).
4. Tablet ingestion + mod parser (the 8 types, prefix/suffix stat IDs).
5. Regression + UI: sortable table (Mod | Avg Value in Exalted | Sample Size | Confidence),
   "last updated" timestamp, rate-limit-respecting Refresh button.

## Open question at handoff time
The user wants to **see/drive this session from their phone** via the Claude Code mobile app
(the app shows cloud "remote" sessions). A local CLI session on the Windows PC does not appear
there on its own. Bridging it needs the code in a **GitHub repo** + continuing in a **cloud
session**. Caveat: it's a Windows desktop app, so final packaging/run happens back on the PC;
the cross-platform code (Rust client, regression, React UI) is fine to build in the cloud.
Resolve the GitHub question with the user before moving the build to the cloud.
