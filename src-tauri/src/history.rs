//! Local time-series of mod values. Each fresh compute appends one slim snapshot (a JSON
//! line), so the UI can show how a mod's value and a type's supply trend over days. This is
//! market data, not personal — safe to persist (see docs/privacy.md). History only grows
//! when a fresh valuation is computed (app refresh, or a scheduled snapshot).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::valuation::Valuation;

#[derive(Serialize, Deserialize)]
struct ModSnap {
    h: String,
    v: f64,
}

#[derive(Serialize, Deserialize)]
struct TypeSnap {
    t: String,
    supply: Option<u64>,
    mods: Vec<ModSnap>,
}

#[derive(Serialize, Deserialize)]
struct Snapshot {
    at: String,
    types: Vec<TypeSnap>,
}

/// One point in a mod's value history.
#[derive(Serialize, Deserialize, Clone)]
pub struct HistoryPoint {
    pub at: String,
    pub value_exalted: f64,
}

/// Append one slim snapshot of a valuation as a JSON line.
pub fn append(path: &Path, v: &Valuation) {
    let snap = Snapshot {
        at: v.updated_at.clone(),
        types: v
            .types
            .iter()
            .map(|t| TypeSnap {
                t: t.tablet_type.clone(),
                supply: t.listings_available,
                mods: t
                    .mods
                    .iter()
                    .map(|m| ModSnap {
                        h: m.stat_hash.clone(),
                        v: m.value_exalted,
                    })
                    .collect(),
            })
            .collect(),
    };
    if let Ok(line) = serde_json::to_string(&snap) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = writeln!(f, "{line}");
        }
    }
}

/// The value time-series for one (type, mod) across all stored snapshots, oldest first.
pub fn series(path: &Path, tablet_type: &str, stat_hash: &str) -> Vec<HistoryPoint> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in text.lines() {
        let Ok(snap) = serde_json::from_str::<Snapshot>(line) else {
            continue;
        };
        if let Some(ts) = snap.types.iter().find(|t| t.t == tablet_type) {
            if let Some(m) = ts.mods.iter().find(|m| m.h == stat_hash) {
                out.push(HistoryPoint {
                    at: snap.at.clone(),
                    value_exalted: m.v,
                });
            }
        }
    }
    out
}
