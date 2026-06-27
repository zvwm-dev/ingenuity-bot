//! Typed views over the parts of the trade API response we care about.
//!
//! The API returns far more than this; we deserialize only the fields the valuator
//! needs and ignore the rest. Parsing is intentionally lenient (`#[serde(default)]`)
//! so a single unexpected field never breaks a whole batch.

use serde::{Deserialize, Serialize};

/// Response from `POST /search/{league}`: a search id plus the matching item ids.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub id: String,
    #[serde(default)]
    pub result: Vec<String>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub complexity: Option<u64>,
}

/// One listing from `GET /fetch/{ids}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Listing {
    pub listing: ListingMeta,
    pub item: Item,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingMeta {
    #[serde(default)]
    pub price: Option<Price>,
    #[serde(default)]
    pub indexed: Option<String>,
}

/// A listed price, e.g. { amount: 2.0, currency: "exalted" }.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Price {
    #[serde(default)]
    pub amount: f64,
    #[serde(default)]
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Item {
    #[serde(rename = "typeLine", default)]
    pub type_line: String,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(rename = "explicitMods", default)]
    pub explicit_mods: Vec<ExplicitMod>,
}

/// A single explicit modifier on a tablet. PoE2's trade2 API returns these enriched
/// with a stable `hash` (stat id), the rendered `description`, and per-roll detail.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplicitMod {
    #[serde(default)]
    pub description: String,
    /// Stable stat id, e.g. "stat.explicit.stat_689816330".
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub mods: Vec<ModRoll>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModRoll {
    /// Affix name, e.g. "Challenger's" (prefix) or "of Shrines" (suffix).
    #[serde(default)]
    pub name: Option<String>,
    /// Tier code: "P1".. for prefixes, "S1".. for suffixes.
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub magnitudes: Vec<Magnitude>,
}

/// The tier's value range for a mod. Note these are the *tier* bounds (strings in the
/// API); the actual rolled value is embedded in `ExplicitMod::description`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Magnitude {
    #[serde(default)]
    pub min: Option<String>,
    #[serde(default)]
    pub max: Option<String>,
}

impl ModRoll {
    /// True if this roll is a prefix (tier code starts with 'P').
    pub fn is_prefix(&self) -> bool {
        matches!(self.tier.as_deref(), Some(t) if t.starts_with('P'))
    }

    /// True if this roll is a suffix (tier code starts with 'S').
    pub fn is_suffix(&self) -> bool {
        matches!(self.tier.as_deref(), Some(t) if t.starts_with('S'))
    }
}
