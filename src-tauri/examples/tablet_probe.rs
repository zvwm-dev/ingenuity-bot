//! Live end-to-end probe of the trade client. Hits the real PoE2 trade API (read-only,
//! rate-limited) and prints what comes back, so we can verify the whole pipeline.
//!
//! Usage:
//!   cargo run --example tablet_probe                       # default league + Breach Tablet
//!   cargo run --example tablet_probe -- "Standard" "Ritual Tablet"

use app_lib::trade::TradeClient;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let league = args.next().unwrap_or_else(|| "Runes of Aldur".to_string());
    let base_type = args.next().unwrap_or_else(|| "Breach Tablet".to_string());

    println!("── ingenuity trade probe ──");
    println!("league: {league}");
    println!("type:   {base_type} (magic rarity)\n");

    let client = match TradeClient::new(league) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to build client: {e}");
            std::process::exit(1);
        }
    };

    match client.fetch_tablet_listings(&base_type, Some("magic"), 10).await {
        Ok(listings) => {
            println!("fetched {} magic listing(s):\n", listings.len());
            for (i, l) in listings.iter().enumerate() {
                let price = l
                    .listing
                    .price
                    .as_ref()
                    .map(|p| format!("{} {}", p.amount, p.currency))
                    .unwrap_or_else(|| "(no price)".to_string());
                println!("{}. {} — {}", i + 1, l.item.type_line, price);
                for m in &l.item.explicit_mods {
                    let tier = m
                        .mods
                        .first()
                        .and_then(|r| r.tier.clone())
                        .unwrap_or_else(|| "?".to_string());
                    println!("     [{tier}] {}", m.description);
                }
            }
            println!("\nOK: pipeline works (search -> rate-limited fetch -> parsed mods + prices).");
        }
        Err(e) => {
            eprintln!("probe failed: {e}");
            std::process::exit(1);
        }
    }
}
