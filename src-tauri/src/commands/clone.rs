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

    // Resolve script directory: in dev, use repo root's py/ folder; in prod, use app bundle Resources
    let script_dir = std::env::current_exe()
        .ok()
        .and_then(|p| {
            // In dev: exe is at src-tauri/target/debug/printables-offline
            // Go up: debug/ -> target/ -> src-tauri/ -> repo root, then join "py"
            let dev_path = p.parent()
                .and_then(|d| d.parent())
                .and_then(|t| t.parent())
                .and_then(|s| s.parent())
                .map(|r| r.join("py"));
            if let Some(ref path) = dev_path {
                if path.exists() {
                    return Some(path.clone());
                }
            }

            // In prod (.app bundle): exe is at Contents/MacOS/printables-offline
            // py/ should be at Contents/Resources/py/
            let resources_path = p.parent()
                .and_then(|macos| macos.parent())  // Contents/
                .map(|contents| contents.join("Resources/py"));
            if let Some(ref path) = resources_path {
                if path.exists() {
                    return Some(path.clone());
                }
            }

            // Fallback: relative to exe
            p.parent().map(|parent| parent.join("../py"))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("./py"));

    let job_id = spawn_clone(&app, python_bin, script_dir, args.url, library.clone(), false)?;

    Ok(CloneResult { job_id })
}
