//! Header-driven rate limiter for the PoE2 trade API.
//!
//! GGG advertises its current limits on every response via headers like:
//!   X-Rate-Limit-Rules: Ip
//!   X-Rate-Limit-Ip:        5:10:60,15:60:300,30:300:1800   (max : period_s : restrict_s)
//!   X-Rate-Limit-Ip-State:  1:10:0,1:60:0,1:300:0           (current usage)
//!   Retry-After: 30                                          (only when throttled)
//!
//! This limiter parses those limit definitions and proactively throttles so we stay
//! within the advertised windows, rather than blindly sleeping a fixed amount. It also
//! hard-blocks for the full `Retry-After` duration if we ever get a 429. The limits can
//! change at any time, so we re-read them from every response.

use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// A single advertised window, e.g. "5 hits per 10 seconds".
#[derive(Clone, Debug)]
struct Bucket {
    max: u32,
    period: Duration,
}

#[derive(Default)]
struct PolicyState {
    buckets: Vec<Bucket>,
    /// Timestamps of recent requests, oldest first.
    hits: VecDeque<Instant>,
    /// If set and in the future, all requests under this policy must wait until then.
    blocked_until: Option<Instant>,
}

/// Throttles outbound requests per logical policy key (e.g. "search", "fetch").
pub struct RateLimiter {
    policies: Mutex<HashMap<String, PolicyState>>,
    /// GGG restricts by IP across ALL endpoints, so a 429 anywhere blocks everything.
    global_block: Mutex<Option<Instant>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            policies: Mutex::new(HashMap::new()),
            global_block: Mutex::new(None),
        }
    }

    /// Seed a policy with conservative default limits, used before we've seen a real
    /// response for that policy. Does nothing if limits are already known.
    pub async fn seed(&self, key: &str, spec: &str) {
        let mut policies = self.policies.lock().await;
        let state = policies.entry(key.to_string()).or_default();
        if state.buckets.is_empty() {
            state.buckets = parse_buckets(spec);
        }
    }

    /// Block until it is safe to make a request under `key`, then record the request.
    pub async fn acquire(&self, key: &str) {
        loop {
            // A global IP restriction (set on any 429) blocks every policy.
            let global_wait = {
                let g = self.global_block.lock().await;
                g.and_then(|until| {
                    let now = Instant::now();
                    (now < until).then(|| until.saturating_duration_since(now))
                })
            };
            if let Some(d) = global_wait {
                if !d.is_zero() {
                    tokio::time::sleep(d).await;
                    continue;
                }
            }

            let wait = {
                let mut policies = self.policies.lock().await;
                let state = policies.entry(key.to_string()).or_default();
                let now = Instant::now();

                // Honor a hard block (Retry-After / restriction) first.
                if let Some(until) = state.blocked_until {
                    if now < until {
                        Some(until.saturating_duration_since(now))
                    } else {
                        state.blocked_until = None;
                        bucket_wait(state, now)
                    }
                } else {
                    bucket_wait(state, now)
                }
            };

            match wait {
                Some(d) if !d.is_zero() => tokio::time::sleep(d).await,
                _ => {
                    let mut policies = self.policies.lock().await;
                    let state = policies.entry(key.to_string()).or_default();
                    let now = Instant::now();
                    state.hits.push_back(now);
                    prune(state, now);
                    return;
                }
            }
        }
    }

    /// Update a policy's known limits from response headers, and apply a hard block if
    /// `retry_after` is present.
    pub async fn update_from_headers(
        &self,
        key: &str,
        limit_spec: Option<&str>,
        retry_after: Option<u64>,
    ) {
        {
            let mut policies = self.policies.lock().await;
            let state = policies.entry(key.to_string()).or_default();
            if let Some(spec) = limit_spec {
                let parsed = parse_buckets(spec);
                if !parsed.is_empty() {
                    state.buckets = parsed;
                }
            }
            if let Some(secs) = retry_after {
                state.blocked_until = Some(Instant::now() + Duration::from_secs(secs));
            }
        } // release the policies lock before taking the global lock

        // A 429 is an IP-wide restriction: block every policy until it clears.
        if let Some(secs) = retry_after {
            let until = Instant::now() + Duration::from_secs(secs);
            let mut g = self.global_block.lock().await;
            if g.map_or(true, |prev| until > prev) {
                *g = Some(until);
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute how long we must wait before another request fits within every bucket.
/// Returns None if a request can be made right now.
fn bucket_wait(state: &PolicyState, now: Instant) -> Option<Duration> {
    let mut wait = Duration::ZERO;
    for bucket in &state.buckets {
        let in_window = |t: &Instant| now.saturating_duration_since(*t) < bucket.period;
        let count = state.hits.iter().filter(|t| in_window(t)).count() as u32;
        if count >= bucket.max {
            // We must wait for the oldest request in this window to age out.
            if let Some(oldest) = state.hits.iter().find(|t| in_window(t)) {
                let age = now.saturating_duration_since(*oldest);
                // Add a small cushion so we don't race the boundary.
                let needed = bucket.period.saturating_sub(age) + Duration::from_millis(100);
                if needed > wait {
                    wait = needed;
                }
            }
        }
    }
    (!wait.is_zero()).then_some(wait)
}

/// Drop timestamps older than the largest tracked window.
fn prune(state: &mut PolicyState, now: Instant) {
    let Some(max_period) = state.buckets.iter().map(|b| b.period).max() else {
        return;
    };
    while let Some(front) = state.hits.front() {
        if now.saturating_duration_since(*front) > max_period {
            state.hits.pop_front();
        } else {
            break;
        }
    }
}

/// Parse "5:10:60,15:60:300" into buckets (max : period_seconds : restrict_seconds).
fn parse_buckets(spec: &str) -> Vec<Bucket> {
    spec.split(',')
        .filter_map(|part| {
            let mut fields = part.split(':');
            let max: u32 = fields.next()?.trim().parse().ok()?;
            let period: u64 = fields.next()?.trim().parse().ok()?;
            Some(Bucket {
                max,
                period: Duration::from_secs(period),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ggg_spec() {
        let b = parse_buckets("5:10:60,15:60:300,30:300:1800");
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].max, 5);
        assert_eq!(b[0].period, Duration::from_secs(10));
        assert_eq!(b[2].max, 30);
        assert_eq!(b[2].period, Duration::from_secs(300));
    }

    #[tokio::test]
    async fn first_requests_do_not_block() {
        let rl = RateLimiter::new();
        rl.seed("search", "5:10:60").await;
        let start = Instant::now();
        for _ in 0..5 {
            rl.acquire("search").await;
        }
        // 5 allowed in the window; should be effectively instant.
        assert!(start.elapsed() < Duration::from_millis(200));
    }

    #[tokio::test]
    async fn retry_after_blocks_all_policies() {
        let rl = RateLimiter::new();
        // A 429 on the search policy must also hold back fetch (IP-wide restriction).
        rl.update_from_headers("search", None, Some(1)).await;
        let start = Instant::now();
        rl.acquire("fetch").await;
        assert!(start.elapsed() >= Duration::from_millis(800));
    }
}
