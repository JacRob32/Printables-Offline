use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_store::StoreExt;
use crate::models::AppPrefs;

const STORE_PATH: &str = "prefs.json";

#[derive(Debug, Deserialize)]
pub struct SetPrefsArgs {
    pub theme: Option<String>,
    pub slicer_key: Option<String>,
    pub slicer_executable: Option<String>,
    pub library_folder: Option<String>,
    pub python_path: Option<String>,
}

/// Get current preferences from the store.
#[tauri::command]
pub fn get_prefs(app: AppHandle) -> Result<AppPrefs, String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;
    let prefs: AppPrefs = store
        .get("app_prefs")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    Ok(prefs)
}

/// Update one or more preference fields. Returns the merged result.
#[tauri::command]
pub fn set_prefs(
    app: AppHandle,
    args: SetPrefsArgs,
) -> Result<AppPrefs, String> {
    let store = app.store(STORE_PATH).map_err(|e| e.to_string())?;

    let mut prefs: AppPrefs = store
        .get("app_prefs")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    if let Some(t) = args.theme { prefs.theme = t; }
    if let Some(s) = args.slicer_key { prefs.slicer_key = s; }
    if let Some(e) = args.slicer_executable { prefs.slicer_executable = Some(e); }
    if let Some(l) = args.library_folder {
        prefs.library_folder = Some(l.clone());
        // Extend runtime asset scope so images in the new folder are servable
        if let Ok(path) = std::path::PathBuf::from(l).canonicalize() {
            let _ = app.asset_protocol_scope().allow_directory(&path, true);
        }
    }
    if let Some(p) = args.python_path { prefs.python_path = Some(p); }

    let val = serde_json::to_value(&prefs).map_err(|e| e.to_string())?;
    store.set("app_prefs", val);
    store.save().map_err(|e| e.to_string())?;

    Ok(prefs)
}
