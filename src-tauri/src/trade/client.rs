//! ToS-compliant client for the PoE2 trade API (the endpoints behind
//! https://www.pathofexile.com/trade2/).
//!
//! Compliance, baked in (see docs/tos-compliance.md):
//! - Sends a descriptive, contactable `User-Agent` on every request.
//! - Reads GGG's `X-Rate-Limit-*` / `Retry-After` headers and throttles via
//!   [`RateLimiter`]; never a fixed sleep-and-pray.
//! - Read-only. No automation of any in-game action.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

use super::error::{Result, TradeError};
use super::models::{Listing, SearchResponse};
use super::rate_limit::RateLimiter;

/// Identifies the app and a reachable contact, as GGG requires.
pub const USER_AGENT: &str = "OAuth ingenuity-bot/0.1.0 (contact: boyd.jordan.t@gmail.com)";

const BASE: &str = "https://www.pathofexile.com/api/trade2";
const FETCH_CHUNK: usize = 10;

// Conservative seed limits (confirmed live 2026-06) used before the first response
// for a policy teaches us the real, current limits.
const SEARCH_LIMIT_SEED: &str = "5:10:60,15:60:300,30:300:1800";
const FETCH_LIMIT_SEED: &str = "12:6:0,16:14:0";

/// A rate-limited handle to the trade API for one league.
pub struct TradeClient {
    http: reqwest::Client,
    limiter: Arc<RateLimiter>,
    league: String,
}

impl TradeClient {
    pub fn new(league: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .gzip(true)
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self {
            http,
            limiter: Arc::new(RateLimiter::new()),
            league: league.into(),
        })
    }

    pub fn league(&self) -> &str {
        &self.league
    }

    /// Step 1: run a search. Returns the search id and the matching item ids.
    pub async fn search(&self, query: Value) -> Result<SearchResponse> {
        self.limiter.seed("search", SEARCH_LIMIT_SEED).await;
        self.limiter.acquire("search").await;

        let url = format!("{BASE}/search/{}", urlencoding::encode(&self.league));
        let resp = self.http.post(&url).json(&query).send().await?;
        self.absorb_headers("search", &resp).await;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let secs = retry_after(&resp);
            self.limiter
                .update_from_headers("search", None, Some(secs))
                .await;
            return Err(TradeError::RateLimited(secs));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(TradeError::Status {
                status: status.as_u16(),
                body: truncate(&body, 300),
            });
        }

        resp.json::<SearchResponse>()
            .await
            .map_err(|e| TradeError::Parse(e.to_string()))
    }

    /// Step 2: fetch full listings for the given item ids (chunked to the API's max of
    /// 10 per request). Unparseable individual listings are skipped, not fatal.
    pub async fn fetch(&self, ids: &[String], query_id: &str) -> Result<Vec<Listing>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(FETCH_CHUNK) {
            self.limiter.seed("fetch", FETCH_LIMIT_SEED).await;
            self.limiter.acquire("fetch").await;

            let url = format!("{BASE}/fetch/{}?query={query_id}", chunk.join(","));
            let resp = self.http.get(&url).send().await?;
            self.absorb_headers("fetch", &resp).await;

            let status = resp.status();
            if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                let secs = retry_after(&resp);
                self.limiter
                    .update_from_headers("fetch", None, Some(secs))
                    .await;
                return Err(TradeError::RateLimited(secs));
            }
            if !status.is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(TradeError::Status {
                    status: status.as_u16(),
                    body: truncate(&body, 300),
                });
            }

            let value: Value = resp
                .json()
                .await
                .map_err(|e| TradeError::Parse(e.to_string()))?;
            if let Some(arr) = value.get("result").and_then(|r| r.as_array()) {
                for entry in arr {
                    if entry.is_null() {
                        continue; // id had no live listing
                    }
                    if let Ok(listing) = serde_json::from_value::<Listing>(entry.clone()) {
                        out.push(listing);
                    }
                }
            }
        }
        Ok(out)
    }

    /// High-level convenience: search a tablet base type and fetch up to `limit` listings.
    /// Pass `rarity = Some("magic")` to restrict to the rollable tablets the valuator wants.
    pub async fn fetch_tablet_listings(
        &self,
        base_type: &str,
        rarity: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Listing>> {
        let mut query = json!({
            "query": { "status": { "option": "online" }, "type": base_type },
            "sort": { "price": "asc" }
        });
        if let Some(r) = rarity {
            query["query"]["filters"] = json!({
                "type_filters": { "filters": { "rarity": { "option": r } } }
            });
        }

        let search = self.search(query).await?;
        let ids: Vec<String> = search.result.into_iter().take(limit).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.fetch(&ids, &search.id).await
    }

    /// Read the current advertised limits off a response and feed them to the limiter.
    async fn absorb_headers(&self, key: &str, resp: &reqwest::Response) {
        let headers = resp.headers();
        // The first rule name, e.g. "Ip" -> header "X-Rate-Limit-Ip".
        let rule = headers
            .get("x-rate-limit-rules")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.split(',').next())
            .map(|s| s.trim().to_lowercase());
        let limit_spec = rule
            .as_ref()
            .and_then(|r| headers.get(format!("x-rate-limit-{r}").as_str()))
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());
        self.limiter
            .update_from_headers(key, limit_spec.as_deref(), None)
            .await;
    }
}

fn retry_after(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(60)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
