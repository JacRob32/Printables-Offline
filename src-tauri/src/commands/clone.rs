use serde::Deserialize;
use std::sync::Mutex;
use tauri::{AppHandle, State};
use crate::models::AppPrefs;
use crate::printables::clone::spawn_clone;

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

    let job_id = spawn_clone(&app, args.url, library.clone())?;

    Ok(CloneResult { job_id })
}
