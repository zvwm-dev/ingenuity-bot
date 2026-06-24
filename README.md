# ingenuity-bot

A desktop tool for **Path of Exile 2** that estimates the in-game value of endgame
Atlas **Precursor Tablet** modifiers from live trade-market data.

Its first feature is a **Tablet Mod Valuator**: it reads current tablet listings from the
official PoE2 trade API, then uses a statistical model to estimate the average value of
each tablet modifier (in Exalted Orbs), so you can see at a glance which mods are worth the
most. Built to be respectful of GGG's Terms of Service — read-only, rate-limit-compliant,
no automation of any in-game actions.

> This product isn't affiliated with or endorsed by Grinding Gear Games in any way.

## Status
Early development. See [HANDOFF.md](HANDOFF.md) for the full plan, research findings, and
the constraints this project is built under.

## Stack
- **Tauri 2** desktop shell (Windows-first; the same UI can later be served as a website)
- **React + TypeScript + Tailwind** frontend
- **Rust** backend (trade-API client, rate limiter, caching)

## Privacy
No personal data is persisted. See [docs/privacy.md](docs/privacy.md) (added during build).
