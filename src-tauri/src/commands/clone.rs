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
            // In prod: exe is in app bundle, py/ is at ../Resources/py or ../py
            // In dev: exe is in target/debug/, py/ is at ../../py
            p.parent()
                .and_then(|parent| {
                    // Try prod layout first (exe in bundle)
                    let prod_path = parent.join("../py");
                    if prod_path.exists() {
                        return Some(prod_path);
                    }
                    // Try dev layout (exe in target/debug/)
                    parent.parent().map(|p| p.join("py"))
                })
        })
        .unwrap_or_else(|| std::path::PathBuf::from("./py"));

    let job_id = spawn_clone(&app, python_bin, script_dir, args.url, library.clone(), false)?;

    Ok(CloneResult { job_id })
}
