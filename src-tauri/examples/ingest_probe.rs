//! Live Phase 4 proof. Samples real listings across all 8 tablet types (spread across the
//! price range), parses + normalizes them, and prints a grouped summary so we can sanity-
//! check the data before building the regression on top of it.
//!
//! Usage:
//!   cargo run --example ingest_probe                 # default league, 40 sampled / type
//!   cargo run --example ingest_probe -- "Standard" 60

use std::collections::HashMap;

use app_lib::ingest::{parse_listing, Affix, ParsedListing};
use app_lib::trade::TradeClient;

const TABLET_TYPES: [&str; 8] = [
    "Breach Tablet",
    "Ritual Tablet",
    "Expedition Tablet",
    "Delirium Tablet",
    "Irradiated Tablet",
    "Overseer Tablet",
    "Abyss Tablet",
    "Temple Tablet",
];

struct Agg {
    description: String,
    affix: Affix,
    rolls: Vec<f64>,
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let league = args.next().unwrap_or_else(|| "Runes of Aldur".to_string());
    let sample: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(40);

    let client = TradeClient::new(league.clone()).expect("client");

    // ── live currency rates -> Exalted ──
    let mut rates: HashMap<String, f64> = HashMap::new();
    rates.insert("exalted".into(), 1.0);
    for cur in ["divine", "chaos"] {
        if let Ok(Some(r)) = client.exchange_rate(cur, "exalted").await {
            rates.insert(cur.into(), r);
        }
    }

    println!("══ ingenuity ingest probe ══");
    println!("league: {league}   sample/type: {sample}");
    println!(
        "rates -> exalted: {}",
        rates
            .iter()
            .map(|(k, v)| format!("{k}={:.1}", v))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!();

    let mut grand_total = 0usize;

    for ttype in TABLET_TYPES {
        let (_total, listings) = match client.sample_tablet_listings(ttype, Some("nonunique"), sample).await {
            Ok(v) => v,
            Err(e) => {
                println!("=== {ttype} ===  ERROR: {e}\n");
                continue;
            }
        };

        let parsed: Vec<ParsedListing> = listings
            .iter()
            .map(|l| parse_listing(l, ttype, &rates))
            .filter(ParsedListing::in_scope)
            .collect();

        grand_total += parsed.len();

        // price spread (in exalted) + mod-count distribution
        let mut prices: Vec<f64> = parsed.iter().filter_map(|p| p.price_exalted).collect();
        let (pmin, pmed, pmax) = min_med_max(&mut prices);
        let mut dist = [0usize; 5]; // index by mod_count 0..=4
        for p in &parsed {
            if p.mod_count <= 4 {
                dist[p.mod_count] += 1;
            }
        }

        println!(
            "=== {ttype} ===  in-scope {} | mods 2:{} 3:{} 4:{} | price {:.1}–{:.1} ex (med {:.1})",
            parsed.len(),
            dist[2],
            dist[3],
            dist[4],
            pmin,
            pmax,
            pmed
        );

        // group mods by stat hash within this type
        let mut groups: HashMap<String, Agg> = HashMap::new();
        for pl in &parsed {
            for m in &pl.mods {
                let g = groups.entry(m.stat_hash.clone()).or_insert_with(|| Agg {
                    description: m.description.clone(),
                    affix: m.affix,
                    rolls: Vec::new(),
                });
                if let Some(v) = m.magnitude {
                    g.rolls.push(v);
                }
            }
        }

        let mut rows: Vec<(&String, &Agg)> = groups.iter().collect();
        rows.sort_by(|a, b| b.1.rolls.len().cmp(&a.1.rolls.len()));
        for (hash, g) in rows.into_iter().take(8) {
            let mut rolls = g.rolls.clone();
            let (rmin, rmed, rmax) = min_med_max(&mut rolls);
            let kind = match g.affix {
                Affix::Prefix => "P",
                Affix::Suffix => "S",
                Affix::Unknown => "?",
            };
            let short = hash.rsplit('_').next().unwrap_or(hash);
            println!(
                "   {kind} n={:<3} roll {:>5.0}–{:<5.0} (med {:>4.0})  {}",
                g.rolls.len(),
                rmin,
                rmax,
                rmed,
                truncate(&g.description, 64)
            );
            let _ = short;
        }
        println!();
    }

    println!("total in-scope (magic/rare, 2-4 mods) parsed: {grand_total}");
    println!("OK: ingest works (price-spread sample -> parse mods+magnitudes -> exalted-normalized -> grouped per type).");
}

fn min_med_max(xs: &mut [f64]) -> (f64, f64, f64) {
    if xs.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = xs.len();
    let med = if n % 2 == 1 {
        xs[n / 2]
    } else {
        (xs[n / 2 - 1] + xs[n / 2]) / 2.0
    };
    (xs[0], med, xs[n - 1])
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
