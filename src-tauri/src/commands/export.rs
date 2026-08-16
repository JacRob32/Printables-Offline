use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;
use tauri_plugin_dialog::DialogExt;
use crate::models::AppPrefs;

#[derive(Debug, Deserialize)]
pub struct ExportArgs {
    pub model_id: String,
}

#[derive(Debug, serde::Serialize)]
pub struct ExportResult {
    pub destination: String,
    pub copied: usize,
}

/// Resolve the absolute path of one model's directory by walking the library.
fn model_dir_from_id(prefs: &AppPrefs, model_id: &str) -> Result<PathBuf, String> {
    let lib = prefs
        .library_folder
        .as_ref()
        .ok_or("Library folder not configured. Set it in Preferences first.")?;
    let lib_path = Path::new(lib);

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
        if meta.model_id == model_id {
            return Ok(entry.path());
        }
    }

    Err(format!("Model not found in library: {model_id}"))
}

/// Copy the model's files out of the library to a user-chosen folder.
/// Shows a native folder picker, copies files/ + images/, then reveals the destination.
#[tauri::command]
pub fn export_files(
    app: tauri::AppHandle,
    args: ExportArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<ExportResult, String> {
    use tauri_plugin_opener::OpenerExt;

    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let src_dir = model_dir_from_id(&prefs, &args.model_id)?;
    let model_name = src_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| args.model_id.clone());

    // Native folder picker (blocking — command runs on a background thread)
    let dest = app
        .dialog()
        .file()
        .set_title("Export model files to…")
        .blocking_pick_folder()
        .ok_or("Export cancelled.")?;
    let dest_path = PathBuf::from(dest.to_string());

    let target_dir = dest_path.join(&model_name);
    fs::create_dir_all(&target_dir).map_err(|e| format!("Cannot create export folder: {e}"))?;

    let mut copied = 0;
    for sub in ["files", "images"] {
        let src_sub = src_dir.join(sub);
        if !src_sub.is_dir() {
            continue;
        }
        let dst_sub = target_dir.join(sub);
        fs::create_dir_all(&dst_sub).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(&src_sub).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let file_type = entry.file_type().map_err(|e| e.to_string())?;
            if file_type.is_file() {
                let dst = dst_sub.join(entry.file_name());
                fs::copy(entry.path(), &dst).map_err(|e| {
                    format!("Failed copying {}: {e}", entry.path().display())
                })?;
                copied += 1;
            }
        }
    }

    // Also copy metadata.json so the export is self-describing
    let meta_src = src_dir.join("metadata.json");
    if meta_src.exists() {
        fs::copy(&meta_src, target_dir.join("metadata.json")).map_err(|e| e.to_string())?;
    }

    // Reveal the export folder in Finder / Explorer
    let _ = app.opener().reveal_item_in_dir(&target_dir);

    Ok(ExportResult {
        destination: target_dir.display().to_string(),
        copied,
    })
}
