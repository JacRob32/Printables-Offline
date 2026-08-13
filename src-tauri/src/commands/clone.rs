use serde::Deserialize;
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
    prefs: State<'_, AppPrefs>,
) -> Result<CloneResult, String> {
    let library = prefs
        .library_folder
        .as_ref()
        .ok_or("Library folder not configured. Set it in Preferences first.")?;

    let python_bin = resolve_python(prefs.python_path.as_deref());
    let script_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("../py")))
        .unwrap_or_else(|| std::path::PathBuf::from("./py"));

    let job_id = spawn_clone(&app, python_bin, script_dir, args.url, library.clone(), false)?;

    Ok(CloneResult { job_id })
}
