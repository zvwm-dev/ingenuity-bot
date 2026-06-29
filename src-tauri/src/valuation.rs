//! Phase 5 orchestration: pull a price-spread sample for every tablet type, parse it,
//! normalize to Exalted, and fit a per-type valuation. Resilient to a mid-run rate-limit
//! restriction — it skips a type that errors and keeps going (the limiter's global block
//! makes the next type wait out the restriction rather than spamming).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::ingest::{parse_listing, ParsedListing};
use crate::model::{value_type, TypeValuation};
use crate::trade::TradeClient;

pub const TABLET_TYPES: [&str; 8] = [
    "Breach Tablet",
    "Ritual Tablet",
    "Expedition Tablet",
    "Delirium Tablet",
    "Irradiated Tablet",
    "Overseer Tablet",
    "Abyss Tablet",
    "Temple Tablet",
];

/// The full computed result handed to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Valuation {
    pub league: String,
    /// RFC3339 UTC timestamp of when this was computed.
    pub updated_at: String,
    /// Live Divine→Exalted rate (for the UI's currency toggle); None if unavailable.
    pub divine_to_exalted: Option<f64>,
    pub types: Vec<TypeValuation>,
}

/// Compute a fresh valuation by hitting the live trade API.
pub async fn compute(league: &str, sample_per_type: usize) -> Result<Valuation, String> {
    let client = TradeClient::new(league.to_string()).map_err(|e| e.to_string())?;

    // Currency rates -> Exalted.
    let mut rates: HashMap<String, f64> = HashMap::new();
    rates.insert("exalted".into(), 1.0);
    let divine = client.exchange_rate("divine", "exalted").await.ok().flatten();
    if let Some(r) = divine {
        rates.insert("divine".into(), r);
    }
    if let Ok(Some(r)) = client.exchange_rate("chaos", "exalted").await {
        rates.insert("chaos".into(), r);
    }

    let mut types = Vec::new();
    for ttype in TABLET_TYPES {
        let (total, listings) = match client
            .sample_tablet_listings(ttype, Some("nonunique"), sample_per_type)
            .await
        {
            Ok(v) => v,
            Err(e) => {
                log::warn!("valuation: skipping {ttype}: {e}");
                continue; // limiter has set any needed global block; press on
            }
        };
        let parsed: Vec<ParsedListing> = listings
            .iter()
            .map(|l| parse_listing(l, ttype, &rates))
            .filter(ParsedListing::in_scope)
            .collect();
        if let Some(mut v) = value_type(ttype, &parsed) {
            v.listings_available = Some(total);
            types.push(v);
        }
    }

    if types.is_empty() {
        return Err("no tablet data could be valued (the trade API may be rate-limiting; try again shortly)".into());
    }

    Ok(Valuation {
        league: league.to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        divine_to_exalted: divine,
        types,
    })
}
