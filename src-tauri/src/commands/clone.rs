use serde::Deserialize;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use crate::models::AppPrefs;
use crate::python::runner::{resolve_python, spawn_clone};

#[derive(Debug, Deserialize)]
pub struct CloneArgs {
    pub url: String,
}

#[derive(Debug, serde::Serialize)]
pub struct CloneResult {
    pub job_id: String,
}

/// Start cloning a model. Returns immediately with a job_id; progress via events.
#[tauri::command]
pub fn clone_model(
    app: AppHandle,
    args: CloneArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<CloneResult, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let library = prefs
        .library_folder
        .as_ref()
        .ok_or("Library folder not configured. Set it in Preferences first.")?;

    let python_bin = resolve_python(prefs.python_path.as_deref());

    // Resolve script directory: in dev, use repo root's py/ folder; in prod, use exe-relative path
    let script_dir = std::env::current_exe()
        .ok()
        .and_then(|p| {
            // In dev: exe is at src-tauri/target/debug/printables-offline
            // Need to go up 3 levels: debug/ -> target/ -> src-tauri/ -> repo root
            // Then join "py" to get repo_root/py/
            let dev_path = p.parent()      // target/debug/
                .and_then(|d| d.parent())   // target/
                .and_then(|t| t.parent())   // src-tauri/
                .and_then(|s| s.parent())   // repo root (Printables Offline/)
                .map(|r| r.join("py"));
            if let Some(ref path) = dev_path {
                if path.exists() {
                    return Some(path.clone());
                }
            }

            // In prod: exe is in app bundle, py/ is at ../Resources/py or ../py
            p.parent()
                .and_then(|parent| {
                    let prod_path = parent.join("../py");
                    if prod_path.exists() {
                        return Some(prod_path);
                    }
                    None
                })
        })
        .unwrap_or_else(|| std::path::PathBuf::from("./py"));

    let job_id = spawn_clone(&app, python_bin, script_dir, args.url, library.clone(), false)?;

    Ok(CloneResult { job_id })
}
