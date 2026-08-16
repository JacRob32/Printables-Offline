use std::sync::Mutex;
use tauri::State;
use crate::library::indexer;
use crate::models::{AppPrefs, LibraryIndex};

/// Return the full library index (models + totals).
#[tauri::command]
pub fn list_models(prefs: State<'_, Mutex<AppPrefs>>) -> Result<LibraryIndex, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib = prefs
        .library_folder
        .as_ref()
        .ok_or("Library folder not configured.")?;
    indexer::index_library(std::path::Path::new(lib))
}

/// Return just the aggregate totals for the sidebar storage card.
#[tauri::command]
pub fn library_stats(prefs: State<'_, Mutex<AppPrefs>>) -> Result<crate::models::LibraryTotals, String> {
    let idx = list_models(prefs)?;
    Ok(idx.totals)
}

/// Rescan the library folder and re-index all metadata.json files.
#[tauri::command]
pub fn rescan_library(prefs: State<'_, Mutex<AppPrefs>>) -> Result<LibraryIndex, String> {
    list_models(prefs)
}
