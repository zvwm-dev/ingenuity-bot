//! Headless snapshot. Computes the valuation and writes the SAME cache + history files the
//! desktop app uses, then exits. Meant to be run by a daily scheduled task so the value
//! history and supply trends accrue even when the app isn't open.
//!
//! Read-only and rate-limited like the app (one light run per day). No GUI, no user data.
//! Paths mirror Tauri's app dirs on Windows: cache = %LOCALAPPDATA%\<id>, data = %APPDATA%\<id>.
//!
//!   snapshot.exe                       # default league + sample
//!   snapshot.exe "Runes of Aldur" 90

use std::path::Path;

use app_lib::{history, valuation};

const IDENTIFIER: &str = "com.ingenuity.tablets";

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let league = args.next().unwrap_or_else(|| "Runes of Aldur".to_string());
    let sample: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(90);

    eprintln!("[snapshot] computing {league} (sample {sample}/type)…");
    match valuation::compute(&league, sample).await {
        Ok(v) => {
            let safe: String = league
                .chars()
                .map(|c| if c.is_alphanumeric() { c } else { '_' })
                .collect();

            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                let dir = Path::new(&local).join(IDENTIFIER);
                let _ = std::fs::create_dir_all(&dir);
                if let Ok(json) = serde_json::to_string(&v) {
                    let _ = std::fs::write(dir.join(format!("valuation_{safe}.json")), json);
                }
            }
            if let Ok(appdata) = std::env::var("APPDATA") {
                let dir = Path::new(&appdata).join(IDENTIFIER);
                let _ = std::fs::create_dir_all(&dir);
                history::append(&dir.join(format!("history_{safe}.jsonl")), &v);
            }

            eprintln!(
                "[snapshot] ok: {} types valued, updated {}",
                v.types.len(),
                v.updated_at
            );
        }
        Err(e) => {
            eprintln!("[snapshot] failed: {e}");
            std::process::exit(1);
        }
    }
}
