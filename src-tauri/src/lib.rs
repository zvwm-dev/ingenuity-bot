pub mod history;
pub mod ingest;
pub mod model;
pub mod trade;
pub mod valuation;

use std::path::PathBuf;

use tauri::Manager;

use trade::models::Listing;
use trade::TradeClient;
use valuation::Valuation;

/// How long a cached valuation is considered fresh (market data, not personal — safe to
/// persist; see docs/privacy.md). Refreshing only on demand keeps API volume ToS-low.
const CACHE_TTL_SECS: u64 = 30 * 60;
const SAMPLE_PER_TYPE: usize = 90;

/// Compute (or load from cache) the value of every tablet mod, per type, for a league.
/// Pass `refresh = true` to force a live recompute (respects the rate limiter).
#[tauri::command]
async fn value_tablets(
    app: tauri::AppHandle,
    league: String,
    refresh: bool,
) -> Result<Valuation, String> {
    let path = cache_file(&app, &league);

    if !refresh {
        if let Some(cached) = path.as_ref().and_then(|p| load_fresh(p, CACHE_TTL_SECS)) {
            return Ok(cached);
        }
    }

    let valuation = valuation::compute(&league, SAMPLE_PER_TYPE).await?;

    if let Some(p) = &path {
        if let Ok(json) = serde_json::to_string(&valuation) {
            let _ = std::fs::write(p, json);
        }
    }
    // Accrue a history snapshot (only on a fresh compute, never on a cache hit).
    if let Some(hp) = history_file(&app, &league) {
        history::append(&hp, &valuation);
    }
    Ok(valuation)
}

/// The stored value history for one mod (oldest first). Empty until snapshots accrue.
#[tauri::command]
fn mod_history(
    app: tauri::AppHandle,
    league: String,
    tablet_type: String,
    stat_hash: String,
) -> Vec<history::HistoryPoint> {
    match history_file(&app, &league) {
        Some(p) => history::series(&p, &tablet_type, &stat_hash),
        None => Vec::new(),
    }
}

/// Debug helper: raw magic/rare listings for one tablet type (read-only, rate-limited).
#[tauri::command]
async fn fetch_tablet_listings(
    league: String,
    base_type: String,
    limit: usize,
) -> Result<Vec<Listing>, String> {
    let client = TradeClient::new(league).map_err(|e| e.to_string())?;
    client
        .fetch_tablet_listings(&base_type, Some("nonunique"), limit)
        .await
        .map_err(|e| e.to_string())
}

fn safe_name(league: &str) -> String {
    league
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect()
}

fn cache_file(app: &tauri::AppHandle, league: &str) -> Option<PathBuf> {
    let dir = app.path().app_cache_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("valuation_{}.json", safe_name(league))))
}

fn history_file(app: &tauri::AppHandle, league: &str) -> Option<PathBuf> {
    let dir = app.path().app_data_dir().ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir.join(format!("history_{}.jsonl", safe_name(league))))
}

fn load_fresh(path: &PathBuf, ttl_secs: u64) -> Option<Valuation> {
    let meta = std::fs::metadata(path).ok()?;
    let age = meta.modified().ok()?.elapsed().ok()?;
    if age.as_secs() > ttl_secs {
        return None;
    }
    let txt = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&txt).ok()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            value_tablets,
            mod_history,
            fetch_tablet_listings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
