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
const EXCHANGE_LIMIT_SEED: &str = "5:15:60,10:90:300,30:300:1800";

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

    /// Search a tablet base type and fetch a sample of listings spread EVENLY across the
    /// price-sorted results, so we span the whole price range rather than only the
    /// (floor-priced) cheapest. `rarity = Some("nonunique")` covers magic + rare + normal;
    /// callers then filter by mod count.
    pub async fn sample_tablet_listings(
        &self,
        base_type: &str,
        rarity: Option<&str>,
        sample_size: usize,
    ) -> Result<Vec<Listing>> {
        let mut query = json!({
            "query": { "status": { "option": "online" }, "type": base_type },
            "sort": { "price": "asc" }
        });
        if let Some(r) = rarity {
            query["query"]["filters"] =
                json!({ "type_filters": { "filters": { "rarity": { "option": r } } } });
        }

        let search = self.search(query).await?;
        let ids = sample_evenly(&search.result, sample_size);
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.fetch(&ids, &search.id).await
    }

    /// Live exchange rate as "amount of `want` per 1 `have`" via the bulk exchange API.
    /// Returns the median of the cheapest current offers, or None if unavailable.
    pub async fn exchange_rate(&self, have: &str, want: &str) -> Result<Option<f64>> {
        if have == want {
            return Ok(Some(1.0));
        }
        self.limiter.seed("exchange", EXCHANGE_LIMIT_SEED).await;
        self.limiter.acquire("exchange").await;

        let body = json!({
            "query": { "status": { "option": "online" }, "want": [want], "have": [have] },
            "sort": { "have": "asc" },
            "engine": "new"
        });
        let url = format!("{BASE}/exchange/{}", urlencoding::encode(&self.league));
        let resp = self.http.post(&url).json(&body).send().await?;
        self.absorb_headers("exchange", &resp).await;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            let secs = retry_after(&resp);
            self.limiter
                .update_from_headers("exchange", None, Some(secs))
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
        let mut ratios: Vec<f64> = Vec::new();
        if let Some(map) = value.get("result").and_then(|r| r.as_object()) {
            for entry in map.values() {
                if let Some(offer) = entry.pointer("/listing/offers/0") {
                    let have_amt = offer.pointer("/exchange/amount").and_then(|v| v.as_f64());
                    let want_amt = offer.pointer("/item/amount").and_then(|v| v.as_f64());
                    if let (Some(h), Some(w)) = (have_amt, want_amt) {
                        if h > 0.0 {
                            ratios.push(w / h);
                        }
                    }
                }
                if ratios.len() >= 60 {
                    break;
                }
            }
        }
        Ok(robust_rate(ratios))
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
        // Respect char boundaries so we never split a multi-byte char.
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

fn median(xs: &mut [f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = xs.len();
    Some(if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    })
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (((sorted.len() - 1) as f64) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Estimate a currency exchange rate robustly. The bulk exchange is full of bait
/// listings that cluster at an absurdly favorable (~1) ratio and often OUTNUMBER the
/// real offers, so a plain median is wrong. Anchor on a high percentile (near the
/// legitimate market cluster, robust to a few extreme outliers), keep ratios within a
/// band of it, and take the median of those.
fn robust_rate(mut ratios: Vec<f64>) -> Option<f64> {
    ratios.retain(|r| *r > 0.0);
    if ratios.is_empty() {
        return None;
    }
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let anchor = percentile(&ratios, 0.90);
    let (lo, hi) = (anchor * 0.5, anchor * 2.0);
    let mut kept: Vec<f64> = ratios.into_iter().filter(|r| *r >= lo && *r <= hi).collect();
    median(&mut kept).or(Some(anchor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_evenly_spans_the_range() {
        let ids: Vec<String> = (0..100).map(|i| i.to_string()).collect();
        let s = sample_evenly(&ids, 5);
        assert_eq!(s, vec!["0", "20", "40", "60", "80"]);
        // small lists return everything
        assert_eq!(sample_evenly(&ids[..3], 5).len(), 3);
    }

    #[test]
    fn robust_rate_ignores_bait_floor() {
        // 27 bait "1.0" ratios plus the real cluster around ~260-302.
        let mut v = vec![1.0; 27];
        v.extend([200.0, 260.0, 260.0, 260.0, 301.0, 302.0]);
        let r = robust_rate(v).expect("rate");
        assert!((200.0..=320.0).contains(&r), "expected ~market rate, got {r}");
    }
}

/// Pick `k` ids spread evenly across `ids` (which are price-sorted), to span the range.
fn sample_evenly(ids: &[String], k: usize) -> Vec<String> {
    if ids.is_empty() || k == 0 {
        return Vec::new();
    }
    if ids.len() <= k {
        return ids.to_vec();
    }
    let step = ids.len() as f64 / k as f64;
    (0..k)
        .map(|i| ids[((i as f64) * step) as usize].clone())
        .collect()
}
