//! Live Phase 5 proof: compute the full per-type valuation and print it, so we can sanity-
//! check the numbers before the UI depends on them.
//!
//!   cargo run --example value_probe                 # default league, 80 sampled / type
//!   cargo run --example value_probe -- "Standard" 60

use app_lib::valuation;

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let league = args.next().unwrap_or_else(|| "Runes of Aldur".to_string());
    let sample: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(80);

    println!("computing valuation for '{league}' (sample {sample}/type)…\n");

    match valuation::compute(&league, sample).await {
        Ok(v) => {
            println!("updated: {}", v.updated_at);
            println!(
                "divine -> exalted: {}\n",
                v.divine_to_exalted
                    .map(|r| format!("{r:.0}"))
                    .unwrap_or_else(|| "n/a".into())
            );
            for t in &v.types {
                println!(
                    "=== {} ===  listings {} | fit R²={:.2} | base {:.1} ex",
                    t.tablet_type, t.listings_used, t.r2, t.base_value_exalted
                );
                if let Some(note) = &t.note {
                    println!("   ! {note}");
                }
                for m in t.mods.iter().take(6) {
                    let kind = match m.affix {
                        app_lib::ingest::Affix::Prefix => "P",
                        app_lib::ingest::Affix::Suffix => "S",
                        app_lib::ingest::Affix::Unknown => "?",
                    };
                    println!(
                        "   {kind} {:>6.1} ex  [{:>3.0}–{:<3.0}] {:<6} n={:<3} {}",
                        m.value_exalted,
                        m.ci_low,
                        m.ci_high,
                        m.confidence,
                        m.sample_size,
                        truncate(&m.description, 52)
                    );
                }
                println!();
            }
            println!("OK: per-type regression produced mod values with CIs + confidence.");

            // Seed the app's on-disk cache so the desktop app loads instantly (matches
            // Tauri's app_cache_dir = %LOCALAPPDATA%\<identifier> on Windows).
            let safe: String = league
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect();
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let dir = std::path::Path::new(&local).join("com.ingenuity.tablets");
                let _ = std::fs::create_dir_all(&dir);
                let path = dir.join(format!("valuation_{safe}.json"));
                if let Ok(json) = serde_json::to_string(&v) {
                    if std::fs::write(&path, json).is_ok() {
                        println!("cache seeded: {}", path.display());
                    }
                }
            }
            // Append a history snapshot too (app_data_dir = %APPDATA%\<identifier>).
            if let Ok(appdata) = std::env::var("APPDATA") {
                let dir = std::path::Path::new(&appdata).join("com.ingenuity.tablets");
                let _ = std::fs::create_dir_all(&dir);
                let hp = dir.join(format!("history_{safe}.jsonl"));
                app_lib::history::append(&hp, &v);
                println!("history appended: {}", hp.display());
            }
        }
        Err(e) => {
            eprintln!("valuation failed: {e}");
            std::process::exit(1);
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let t: String = s.chars().take(max).collect();
        format!("{t}…")
    }
}
