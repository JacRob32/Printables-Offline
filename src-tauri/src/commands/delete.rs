use serde::Deserialize;
use std::fs;
use std::sync::Mutex;
use tauri::State;
use crate::models::AppPrefs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteModelArgs {
    pub model_id: String,
}

/// Delete a model's directory from the library.
#[tauri::command]
pub fn delete_model(
    args: DeleteModelArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<(), String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib = prefs
        .library_folder
        .as_ref()
        .ok_or("Library folder not configured. Set it in Preferences first.")?;
    let lib_path = std::path::Path::new(lib);

    // Find the model directory by matching model_id in metadata.json
    let mut model_dir = None;
    for entry in fs::read_dir(lib_path).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta_path = entry.path().join("metadata.json");
        if !meta_path.exists() {
            continue;
        }
        let Ok(raw) = fs::read_to_string(&meta_path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<crate::models::MetadataV1>(&raw) else {
            continue;
        };
        if meta.model_id == args.model_id {
            model_dir = Some(entry.path());
            break;
        }
    }

    let dir = model_dir.ok_or(format!("Model not found in library: {}", args.model_id))?;

    // Delete the entire model directory
    fs::remove_dir_all(&dir).map_err(|e| format!("Failed to delete model directory: {e}"))?;

    Ok(())
}
