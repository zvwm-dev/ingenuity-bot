# ToS Compliance

This document records the Grinding Gear Games (GGG) policies this project operates under,
and exactly how the app honors each one. It is intentionally specific so the constraints are
auditable in-repo. **Treat this as binding.**

> This product isn't affiliated with or endorsed by Grinding Gear Games in any way.

## Sources
- GGG Developer Docs / OAuth & API policy: https://www.pathofexile.com/developer/docs
- Authorization (OAuth 2.1, scopes, PKCE): https://www.pathofexile.com/developer/docs/authorization
- Trade-site rate-limit behavior (community-confirmed by GGG staff):
  https://www.pathofexile.com/forum/view-thread/3056323

## What the app does and does not do
- **Reads market data only.** It queries trade listings and currency exchange rates. It
  computes statistics from them. That is the entire scope.
- **No automation of in-game actions.** No auto-trading, no auto-whisper, no sending of
  messages, no interaction with the game client or its files of any kind. GGG terminates
  accounts for tools that interact with the game executable/files (ToS 7b/7c/7i).
- **No reverse-engineering of undocumented endpoints beyond the trade endpoints already used
  by the official trade site**, which is the same surface every mainstream community price-
  checker uses.

## Required behaviors (implemented in the Rust backend)
1. **User-Agent on every request.** Format:
   `OAuth ingenuity-bot/0.1.0 (contact: boyd.jordan.t@gmail.com)`
   Identifies the app and a reachable contact, per GGG's stated requirement.
2. **Header-driven rate limiting.** The client reads `X-Rate-Limit-Policy`,
   `X-Rate-Limit-Rules`, `X-Rate-Limit-{rule}` (format `max-hits:period-seconds:restrict-seconds`)
   and `Retry-After` on every response, and throttles to stay within the *advertised* limits
   rather than guessing. Known trade limits as of 2026-06 (IP-based):
   - search: 5/12s, 15/62s, 30/302s
   - fetch: 12/6s, 16/14s
   - exchange: 5/17s, 10/92s, 30/302s
   On `429` or a stated restriction, the client backs off for the full `Retry-After` window.
3. **Aggressive caching.** Market data is cached locally and only refreshed on explicit user
   action (the Refresh button) or after a sensible TTL — never in a polling loop. This
   minimizes request volume, which is both the ToS-friendly and the rate-limit-friendly choice.
   The optional daily snapshot task (`snapshot.exe`, run by Windows Task Scheduler) performs
   **at most one compliant refresh per day** to accrue price history — a single low-volume run,
   not continuous polling.
4. **Minimal scope.** The tablet valuator uses anonymous trade search (no auth, no scopes).
   Optional login, when added, requests only `account:profile`. See `docs/privacy.md`.

## Required disclosure
The GGG non-affiliation disclaimer is shown in the app UI (status bar) and in this repo's
README. Do not remove it.

## If GGG asks us to stop
If GGG revokes access or requests changes, comply immediately. Access to the trade API is a
privilege extended at GGG's discretion, not a right.
