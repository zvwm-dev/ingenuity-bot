# Privacy

ingenuity-bot is built so that **no personal user data is persisted to disk**. This is a
design constraint, not a configuration option.

## What is never written to disk
- Your Path of Exile / GGG account name
- Your character names, stash contents, or any account-identifying information
- Any data tied to your identity

Anything user-specific exists **in memory only**, for the duration of a session, and is gone
when the app closes.

## Authentication tokens
Optional "Log in with Path of Exile" uses GGG's official **OAuth 2.0 (Authorization Code +
PKCE)** flow. You never type your pathofexile.com password into this app.

- Access and refresh tokens are stored in the **operating system keychain** via Tauri's
  secure storage (`tauri-plugin-stronghold`), encrypted at rest.
- Tokens are **never** written to a plaintext file, a log, or the market-data cache.
- The only scope requested is `account:profile` (to display your account name). The core
  tablet valuator requires no login and no scopes at all.

## What IS cached (and why it's fine)
The **market data** — tablet listings, mod prices, currency exchange rates, and the price
history we accumulate over time — is cached locally. This is public market information, not
personal data, and caching it is how we keep API request volume low (see
`docs/tos-compliance.md`). The cache lives under the app's data directory and the `cache/`
path, both gitignored.

## Telemetry
- **None.** No analytics, no usage tracking, no crash reporting that includes user data.
- If crash reporting is ever added, it must be **opt-in** and must scrub any user-specific
  fields before sending. Until then, the app phones home to nothing except the GGG APIs it
  needs to function.
