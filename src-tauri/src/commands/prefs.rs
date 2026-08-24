use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, State};
use crate::models::AppPrefs;

/// Get the prefs file path: ~/.printablesoffline/prefs.json
fn prefs_path() -> Result<PathBuf, String> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map_err(|_| "Cannot determine home directory")?;
    let dir = PathBuf::from(home).join(".printablesoffline");
    fs::create_dir_all(&dir).map_err(|e| format!("Cannot create config directory: {e}"))?;
    Ok(dir.join("prefs.json"))
}

fn load_prefs() -> Result<AppPrefs, String> {
    let path = prefs_path()?;
    if !path.exists() {
        return Ok(AppPrefs::default());
    }
    let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}

fn save_prefs(prefs: &AppPrefs) -> Result<(), String> {
    let path = prefs_path()?;
    let raw = serde_json::to_string_pretty(prefs).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetPrefsArgs {
    pub theme: Option<String>,
    pub slicer_key: Option<String>,
    pub slicer_executable: Option<String>,
    pub library_folder: Option<String>,
    pub python_path: Option<String>,
}

/// Get current preferences.
#[tauri::command]
pub fn get_prefs(_app: AppHandle) -> Result<AppPrefs, String> {
    load_prefs()
}

/// Update one or more preference fields. Returns the merged result.
#[tauri::command]
pub fn set_prefs(
    app: AppHandle,
    args: SetPrefsArgs,
    prefs_state: State<'_, Mutex<AppPrefs>>,
) -> Result<AppPrefs, String> {
    let mut prefs = load_prefs()?;

    if let Some(t) = args.theme { prefs.theme = t; }
    if let Some(s) = args.slicer_key { prefs.slicer_key = s; }
    if let Some(e) = args.slicer_executable { prefs.slicer_executable = Some(e); }
    if let Some(l) = args.library_folder {
        // Create "Printables Offline Library" subdirectory inside the chosen folder
        let lib_path = std::path::PathBuf::from(&l).join("Printables Offline Library");
        let _ = std::fs::create_dir_all(&lib_path);
        prefs.library_folder = Some(lib_path.display().to_string());
        // Extend runtime asset scope so images in the new folder are servable
        if let Ok(path) = lib_path.canonicalize() {
            let _ = app.asset_protocol_scope().allow_directory(&path, true);
        }
    }
    if let Some(p) = args.python_path { prefs.python_path = Some(p); }

    save_prefs(&prefs)?;

    // Update managed state so other commands see the new prefs immediately
    let mut managed = prefs_state.lock().map_err(|e| e.to_string())?;
    *managed = prefs.clone();

    Ok(prefs)
}
