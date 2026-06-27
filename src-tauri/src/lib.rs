pub mod ingest;
pub mod trade;

use trade::models::Listing;
use trade::TradeClient;

/// Fetch live tablet listings for one base type in one league.
///
/// Read-only and rate-limited (see `trade::client`). Defaults to magic-rarity tablets,
/// which are the ones the valuator models. Returns up to `limit` listings.
#[tauri::command]
async fn fetch_tablet_listings(
    league: String,
    base_type: String,
    limit: usize,
) -> Result<Vec<Listing>, String> {
    let client = TradeClient::new(league).map_err(|e| e.to_string())?;
    client
        .fetch_tablet_listings(&base_type, Some("magic"), limit)
        .await
        .map_err(|e| e.to_string())
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
        .invoke_handler(tauri::generate_handler![fetch_tablet_listings])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
