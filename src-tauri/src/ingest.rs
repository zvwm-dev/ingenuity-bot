//! Phase 4: turn raw trade listings into clean, structured observations the valuation
//! model can consume.
//!
//! For each listing we extract, per modifier: the stable stat hash, whether it is a
//! prefix or suffix, the ACTUAL rolled magnitude (parsed out of the rendered text — roll
//! size matters to price), and a human-readable description with GGG's `[tag|display]`
//! markup stripped. Prices are normalized to Exalted via live exchange rates.

use std::collections::HashMap;
use std::sync::OnceLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::trade::models::Listing;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Affix {
    Prefix,
    Suffix,
    Unknown,
}

/// One parsed modifier on a tablet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedMod {
    /// Stable stat id, e.g. "stat.explicit.stat_689816330".
    pub stat_hash: String,
    pub affix: Affix,
    pub tier: Option<String>,
    /// The actual rolled value parsed from the description (None for mods with no number).
    pub magnitude: Option<f64>,
    /// Markup-stripped description, e.g. "Map has 95% increased chance to contain Shrines".
    pub description: String,
}

/// One parsed listing, scoped to the tablet base type it was queried under.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedListing {
    pub tablet_type: String,
    pub rarity: String,
    pub mod_count: usize,
    /// Listed price normalized to Exalted (None if currency rate unknown / no price).
    pub price_exalted: Option<f64>,
    /// Original listed price as (amount, currency), for display/debugging.
    pub price_raw: Option<(f64, String)>,
    pub mods: Vec<ParsedMod>,
}

impl ParsedListing {
    /// True if this is a magic or rare tablet with 2-4 total mods — the modelling scope
    /// Jordan specified (rare gives the mod-combination variety the regression needs).
    pub fn in_scope(&self) -> bool {
        matches!(self.rarity.as_str(), "Magic" | "Rare") && (2..=4).contains(&self.mod_count)
    }
}

/// Parse a raw listing into structured form, tagged with the tablet base type it was
/// queried under (the typeLine itself carries affix names, so we trust the query, not it).
pub fn parse_listing(
    listing: &Listing,
    tablet_type: &str,
    rates: &HashMap<String, f64>,
) -> ParsedListing {
    let rarity = listing.item.rarity.clone().unwrap_or_default();

    let mods: Vec<ParsedMod> = listing
        .item
        .explicit_mods
        .iter()
        .map(|m| {
            let roll = m.mods.first();
            let tier = roll.and_then(|r| r.tier.clone());
            let affix = match tier.as_deref() {
                Some(t) if t.starts_with('P') => Affix::Prefix,
                Some(t) if t.starts_with('S') => Affix::Suffix,
                _ => Affix::Unknown,
            };
            let description = clean_markup(&m.description);
            let magnitude = extract_magnitude(&description);
            ParsedMod {
                stat_hash: m.hash.clone().unwrap_or_default(),
                affix,
                tier,
                magnitude,
                description,
            }
        })
        .collect();

    let price_raw = listing
        .listing
        .price
        .as_ref()
        .map(|p| (p.amount, p.currency.clone()));
    let price_exalted = listing
        .listing
        .price
        .as_ref()
        .and_then(|p| normalize_to_exalted(p.amount, &p.currency, rates));

    ParsedListing {
        tablet_type: tablet_type.to_string(),
        rarity,
        mod_count: mods.len(),
        price_exalted,
        price_raw,
        mods,
    }
}

/// Convert an amount in `currency` to Exalted using a {currency -> exalted-per-unit} map.
pub fn normalize_to_exalted(
    amount: f64,
    currency: &str,
    rates: &HashMap<String, f64>,
) -> Option<f64> {
    rates.get(currency).map(|rate| amount * rate)
}

/// Strip GGG's inline markup: `[Shrine|Shrines]` -> "Shrines", `[Rarity]` -> "Rarity".
pub fn clean_markup(s: &str) -> String {
    static PIPED: OnceLock<Regex> = OnceLock::new();
    static PLAIN: OnceLock<Regex> = OnceLock::new();
    let piped = PIPED.get_or_init(|| Regex::new(r"\[([^\]|]+)\|([^\]]+)\]").unwrap());
    let plain = PLAIN.get_or_init(|| Regex::new(r"\[([^\]]+)\]").unwrap());
    let s = piped.replace_all(s, "$2");
    plain.replace_all(&s, "$1").into_owned()
}

/// Pull the first signed number out of a mod description (the rolled value).
pub fn extract_magnitude(s: &str) -> Option<f64> {
    static NUM: OnceLock<Regex> = OnceLock::new();
    let num = NUM.get_or_init(|| Regex::new(r"-?\d+(?:\.\d+)?").unwrap());
    num.find(s).and_then(|m| m.as_str().parse::<f64>().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_markup() {
        assert_eq!(
            clean_markup("Map has 95% increased chance to contain [Shrine|Shrines]"),
            "Map has 95% increased chance to contain Shrines"
        );
        assert_eq!(
            clean_markup("[Rarity|Unique] Monsters have 1 additional [MonsterModifiers|Modifier]"),
            "Unique Monsters have 1 additional Modifier"
        );
        assert_eq!(clean_markup("9% increased [Rarity] of Items"), "9% increased Rarity of Items");
    }

    #[test]
    fn extracts_rolled_value() {
        assert_eq!(extract_magnitude("35% increased Gold found in Map"), Some(35.0));
        assert_eq!(extract_magnitude("Map contains 2 additional Rare Chests"), Some(2.0));
        assert_eq!(extract_magnitude("Adds a Breach with no number"), None);
    }

    #[test]
    fn normalizes_currency() {
        let mut rates = HashMap::new();
        rates.insert("exalted".to_string(), 1.0);
        rates.insert("divine".to_string(), 300.0);
        assert_eq!(normalize_to_exalted(2.0, "divine", &rates), Some(600.0));
        assert_eq!(normalize_to_exalted(5.0, "exalted", &rates), Some(5.0));
        assert_eq!(normalize_to_exalted(1.0, "mirror", &rates), None);
    }
}
