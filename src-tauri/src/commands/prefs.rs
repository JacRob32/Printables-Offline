use serde::Deserialize;
use tauri::{AppHandle, Manager};
use crate::models::AppPrefs;

const STORE_LABEL: &str = "prefs";

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
    let store = app
        .state::<tauri_plugin_store::StoreCollection>()
        .get(STORE_LABEL)
        .ok_or("Store not initialized")?;
    let locked = store.lock().map_err(|e| e.to_string())?;
    let prefs: AppPrefs = locked
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
    let store = app
        .state::<tauri_plugin_store::StoreCollection>()
        .get(STORE_LABEL)
        .ok_or("Store not initialized")?;
    let mut locked = store.lock().map_err(|e| e.to_string())?;

    let mut prefs: AppPrefs = locked
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
            let _ = app.fs_scope().allow_directory(&path, true);
        }
    }
    if let Some(p) = args.python_path { prefs.python_path = Some(p); }

    let val = serde_json::to_value(&prefs).map_err(|e| e.to_string())?;
    locked.set("app_prefs", val);
    locked.save().map_err(|e| e.to_string())?;

    Ok(prefs)
}
