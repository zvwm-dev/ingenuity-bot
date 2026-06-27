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
- **OAuth email status:** Jordan SENT the registration email to oauth@grindinggear.com on
  2026-06-24 (do NOT prompt him to send it again). Now awaiting GGG's reply — can take weeks.
  Until a client_id arrives, ship a "Login with Path of Exile" button that's present but
  disabled/"coming soon." Architect auth so going live is a one-config change.

## Modeling decisions (from Jordan, the domain expert — 2026-06-26)
Search BOTH magic AND rare tablets, filtered to **2-4 total modifiers** (drop 1-mod and 5-6-mod
outliers). Rationale: rare tablets give varied mod COMBINATIONS, which is what lets the
regression separate individual mod values (pure 2-mod magic data is weakly identifiable).
His answers driving the model:
1. **Combos cost a premium** — certain mod pairings sell above the sum of parts. => the additive
   model is an approximation; report fit quality (R²/CI) and flag that v1 gives average MARGINAL
   mod value, not specific-combo prices. Consider top pairwise interactions later.
2. **Value depends on tablet type** — the same shared mod is worth different amounts per type.
   => model PER TABLET TYPE (8 separate regressions); features are (type × mod), not pooled.
3. **Roll size matters a lot** — higher magnitudes command more. => parse the ACTUAL rolled
   number from explicitMods[].description; regress price on magnitude (coef = exalted per unit);
   report value at a typical/median roll, not just presence.
4. **Currency** — show Exalted by default with a Divine toggle; normalize all prices to Exalted.
Sampling: don't just take the cheapest N (they're floor-priced at 1ex). One search returns the
price-sorted id list + `total`; sample ids EVENLY across that list to span the price range, then
fetch the sample. Get magic+rare via rarity="nonunique" + client-side 2-4 mod-count filter.

## Build plan (phases — check in with user after each)
1. ✅ Research (done — this document).
2. ✅ Scaffold Tauri 2 + React 19 + TS + Tailwind v4. Native window launches & verified on
   Windows (screenshot shown to user). docs/tos-compliance.md + docs/privacy.md written.
   Toolchain installed on this PC: Node 24, Rust 1.96 (stable-msvc), VS2022 C++ Build Tools,
   WebView2. First `npm run tauri dev` compile ~1m27s; binary is `app.exe` (Cargo pkg name
   "app"); window title "ingenuity", 1100x720 dark. NOTE: a GUI app started from a detached
   background shell self-exits — to screenshot it, launch via Start-Job and capture within the
   same foreground script (see session history).
3. ✅ Rate-limited Rust trade-API client. Module `src-tauri/src/trade/` (client, rate_limit,
   models, error). Header-driven RateLimiter reads X-Rate-Limit-{rule} + Retry-After. Search
   -> chunked fetch (10/req), resilient per-listing parse. Tauri command `fetch_tablet_listings`.
   Live probe `cargo run --example tablet_probe` VERIFIED: anonymous search+fetch works (HTTP
   200), pulled 10 real magic Breach Tablets with P1/S1 mods + prices. Unit tests pass.
   KEY DATA SHAPE: fetch listing -> item.typeLine, item.rarity ("Magic"/"Normal"/"Unique"),
   item.explicitMods[] = {description (has the rolled number in text), hash (stat id), mods[]:
   {name, tier ("P*"=prefix/"S*"=suffix), magnitudes[]:{min,max}=TIER range not the roll}}.
   listing.price = {amount, currency}. Tablet base type strings (from /data/items): "Abyss
   Tablet","Breach Tablet","Delirium Tablet","Expedition Tablet","Irradiated Tablet","Temple
   Tablet","Overseer Tablet","Ritual Tablet". Filter rarity via
   query.filters.type_filters.filters.rarity.option="magic". League default "Runes of Aldur".
   NOTE: actual rolled magnitude must be parsed from explicitMods[].description text (Phase 4).
   Caching not yet added — do it in Phase 4/5 (persist market snapshots; see docs/privacy.md).
4. ✅ Tablet ingestion + mod parser. `src-tauri/src/ingest.rs`: parse_listing -> ParsedListing
   {tablet_type, rarity, mod_count, price_exalted, mods:[ParsedMod{stat_hash, affix
   (Prefix/Suffix), tier, magnitude, description}]}. clean_markup strips [tag|display];
   extract_magnitude pulls the rolled number from the text; normalize_to_exalted via rates map.
   in_scope() = magic|rare & 2-4 mods. Client gained: sample_tablet_listings (rarity
   "nonunique" + EVEN sampling across price-sorted ids) and exchange_rate (bulk exchange,
   robust against bait listings). Example `cargo run --example ingest_probe` VERIFIED live: 114
   in-scope listings parsed across 6 types (clean per-type mod groups w/ roll ranges).
   TWO FIXES from that run (both unit-tested): (a) currency — bulk exchange is full of bait
   offers at ~1 ratio that outnumber real ones; robust_rate anchors on the 90th percentile and
   bands around it (divine≈260-302 ex, not 1). (b) rate limiter now applies a GLOBAL block on
   any 429 (GGG restricts per-IP across all endpoints), not just the one policy.
   LESSON: limits are per-IP and persist across process restarts; repeated cold test runs in a
   short window tripped a 600s restriction (limiter correctly backed off). Don't hammer; one
   long-running limiter in the real app won't accumulate this way. NOTE: Abyss/Temple types +
   live currency re-confirm were pending the IP cooldown at commit time — re-verify at Phase 5
   start (which fetches anyway). 8 unit tests green.
5. Regression + UI: sortable table (Mod | Avg Value in Exalted | Sample Size | Confidence),
   "last updated" timestamp, rate-limit-respecting Refresh button.
   Design reference: design/ingenuity-ui-v1.html (user's mockup). Caveats logged: trade API is
   snapshot-only (no 7d history / volume on day one — accrue over time); price unit Exalted not
   Divine; fold sample-size + confidence back in (derive S/A/B/C tier from confidence).

## Open question at handoff time
The user wants to **see/drive this session from their phone** via the Claude Code mobile app
(the app shows cloud "remote" sessions). A local CLI session on the Windows PC does not appear
there on its own. Bridging it needs the code in a **GitHub repo** + continuing in a **cloud
session**. Caveat: it's a Windows desktop app, so final packaging/run happens back on the PC;
the cross-platform code (Rust client, regression, React UI) is fine to build in the cloud.
Resolve the GitHub question with the user before moving the build to the cloud.
