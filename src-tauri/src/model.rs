//! Phase 5: per-tablet-type valuation via least-squares regression.
//!
//! For one tablet type we regress listing price (Exalted) on the magnitude of each mod:
//!     price ≈ base + Σ_j  βⱼ · (rolled magnitude of mod j)
//! So βⱼ is "Exalted per unit of mod j", and βⱼ × (typical roll) is the mod's value at a
//! normal roll. We use an SVD-based least-squares solve so collinear / co-occurring mods
//! (which Jordan flagged — combos cost a premium) degrade gracefully into wide confidence
//! intervals rather than blowing up.
//!
//! Honesty notes baked in:
//! - We report per-type R² so the user can see how well an *additive* model fits (the
//!   combo premium shows up as unexplained variance / low R²).
//! - Each mod carries a sample size and a 95% confidence interval; thin or unstable
//!   estimates are labelled Low confidence rather than presented as fact.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use nalgebra::{DMatrix, DVector};
use serde::{Deserialize, Serialize};

use crate::ingest::{Affix, ParsedListing};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModValue {
    pub stat_hash: String,
    pub description: String,
    pub affix: Affix,
    /// Exalted per unit of the mod's magnitude (the raw regression coefficient).
    pub per_unit_exalted: f64,
    /// Median observed roll, used as the "typical" magnitude.
    pub typical_roll: f64,
    /// Estimated value at a typical roll (per_unit × typical_roll).
    pub value_exalted: f64,
    pub ci_low: f64,
    pub ci_high: f64,
    pub sample_size: usize,
    /// "High" | "Medium" | "Low".
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeValuation {
    pub tablet_type: String,
    pub listings_used: usize,
    /// Goodness of fit for the additive model (0..1). Low => combos/interactions matter.
    pub r2: f64,
    /// Intercept: the baseline value of a tablet of this type before mods.
    pub base_value_exalted: f64,
    pub mods: Vec<ModValue>,
}

struct ModAgg {
    description: String,
    affix: Affix,
    rolls: Vec<f64>,
    count: usize,
}

/// Fit the valuation for a single tablet type. Returns None if there isn't enough data.
pub fn value_type(tablet_type: &str, listings: &[ParsedListing]) -> Option<TypeValuation> {
    // Only listings with a known Exalted price can train the model.
    let data: Vec<&ParsedListing> = listings
        .iter()
        .filter(|l| l.price_exalted.is_some())
        .collect();
    if data.len() < 6 {
        return None;
    }

    // Catalogue the distinct mods (keyed by stat hash, falling back to description).
    let mut aggs: BTreeMap<String, ModAgg> = BTreeMap::new();
    for l in &data {
        for m in &l.mods {
            let key = mod_key(m);
            let agg = aggs.entry(key).or_insert_with(|| ModAgg {
                description: m.description.clone(),
                affix: m.affix,
                rolls: Vec::new(),
                count: 0,
            });
            agg.count += 1;
            if let Some(v) = m.magnitude {
                agg.rolls.push(v);
            }
        }
    }

    let keys: Vec<String> = aggs.keys().cloned().collect();
    let p = keys.len();
    let n = data.len();
    if p == 0 {
        return None;
    }
    let col_of: std::collections::HashMap<&String, usize> =
        keys.iter().enumerate().map(|(i, k)| (k, i + 1)).collect();

    // Design matrix X (intercept + one column per mod) and target y (price).
    let cols = p + 1;
    let mut x = DMatrix::<f64>::zeros(n, cols);
    let mut y = DVector::<f64>::zeros(n);
    for (i, l) in data.iter().enumerate() {
        x[(i, 0)] = 1.0;
        y[i] = l.price_exalted.unwrap();
        for m in &l.mods {
            if let Some(&c) = col_of.get(&mod_key(m)) {
                // Use the rolled magnitude; mods with no number act as presence (1.0).
                x[(i, c)] = m.magnitude.unwrap_or(1.0);
            }
        }
    }

    // Least squares via SVD (robust to collinear / co-occurring mods).
    let svd = x.clone().svd(true, true);
    let beta = svd.solve(&y, 1e-9).ok()?;

    // Fit diagnostics.
    let yhat = &x * &beta;
    let resid = &y - &yhat;
    let rss = resid.dot(&resid);
    let ybar = y.mean();
    let tss: f64 = y.iter().map(|v| (v - ybar).powi(2)).sum();
    let r2 = if tss > 0.0 { 1.0 - rss / tss } else { 0.0 };
    let dof = (n as f64 - cols as f64).max(1.0);
    let sigma2 = rss / dof;

    // Coefficient covariance ≈ σ²·(XᵀX)⁺ for standard errors.
    let xtx = x.tr_mul(&x);
    let cov = xtx.pseudo_inverse(1e-9).ok()? * sigma2;

    let mut mods: Vec<ModValue> = keys
        .iter()
        .map(|key| {
            let c = col_of[key];
            let agg = &aggs[key];
            let per_unit = beta[c];
            let se = cov[(c, c)].max(0.0).sqrt();
            let typical = median(&agg.rolls).unwrap_or(1.0);
            let value = per_unit * typical;
            let half_ci = 1.96 * se * typical;
            ModValue {
                stat_hash: key.clone(),
                description: agg.description.clone(),
                affix: agg.affix,
                per_unit_exalted: per_unit,
                typical_roll: typical,
                value_exalted: value,
                ci_low: value - half_ci,
                ci_high: value + half_ci,
                sample_size: agg.count,
                confidence: classify(value, half_ci, agg.count),
            }
        })
        .collect();

    mods.sort_by(|a, b| {
        b.value_exalted
            .partial_cmp(&a.value_exalted)
            .unwrap_or(Ordering::Equal)
    });

    Some(TypeValuation {
        tablet_type: tablet_type.to_string(),
        listings_used: n,
        r2,
        base_value_exalted: beta[0],
        mods,
    })
}

fn mod_key(m: &crate::ingest::ParsedMod) -> String {
    if m.stat_hash.is_empty() {
        m.description.clone()
    } else {
        m.stat_hash.clone()
    }
}

fn classify(value: f64, half_ci: f64, sample: usize) -> String {
    if sample < 5 {
        return "Low".into();
    }
    let rel = if value.abs() > 1e-6 {
        half_ci / value.abs()
    } else {
        f64::INFINITY
    };
    if rel < 0.25 {
        "High".into()
    } else if rel < 0.6 {
        "Medium".into()
    } else {
        "Low".into()
    }
}

fn median(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ingest::{Affix, ParsedListing, ParsedMod};

    fn mk(price: f64, mods: &[(&str, f64)]) -> ParsedListing {
        ParsedListing {
            tablet_type: "Test".into(),
            rarity: "Rare".into(),
            mod_count: mods.len(),
            price_exalted: Some(price),
            price_raw: Some((price, "exalted".into())),
            mods: mods
                .iter()
                .map(|(h, mag)| ParsedMod {
                    stat_hash: (*h).into(),
                    affix: Affix::Prefix,
                    tier: Some("P1".into()),
                    magnitude: Some(*mag),
                    description: format!("{h} mod"),
                })
                .collect(),
        }
    }

    #[test]
    fn recovers_known_mod_values() {
        // Construct data where price = 2*A + 5*B exactly; the model should recover ~2 and ~5.
        let mut data = Vec::new();
        for a in 1..=6 {
            for b in 1..=6 {
                let (af, bf) = (a as f64, b as f64);
                data.push(mk(2.0 * af + 5.0 * bf, &[("A", af), ("B", bf)]));
            }
        }
        let v = value_type("Test", &data).expect("valuation");
        assert!(v.r2 > 0.99, "expected near-perfect fit, got R²={}", v.r2);
        let a = v.mods.iter().find(|m| m.stat_hash == "A").unwrap();
        let b = v.mods.iter().find(|m| m.stat_hash == "B").unwrap();
        assert!((a.per_unit_exalted - 2.0).abs() < 0.01, "A={}", a.per_unit_exalted);
        assert!((b.per_unit_exalted - 5.0).abs() < 0.01, "B={}", b.per_unit_exalted);
    }
}
