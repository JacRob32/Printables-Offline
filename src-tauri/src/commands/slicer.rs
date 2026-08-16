use serde::Deserialize;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::State;
use crate::models::AppPrefs;

#[derive(Debug, Deserialize)]
pub struct OpenInSlicerArgs {
    pub model_id: String,
    pub slicer: String,
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SliceFileArgs {
    pub file: String,
}

/// Launch the configured slicer with the given model files as arguments.
#[tauri::command]
pub fn open_in_slicer(
    args: OpenInSlicerArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<(), String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let exe = prefs
        .slicer_executable
        .as_ref()
        .ok_or("Slicer executable not configured. Set it in Preferences.")?;

    let mut cmd = Command::new(exe);
    cmd.args(&args.files).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("Failed to launch slicer: {e}"))?;
    Ok(())
}

/// Launch the slicer with a single file (quick-slice from card).
#[tauri::command]
pub fn slice_file(
    args: SliceFileArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<(), String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let exe = prefs
        .slicer_executable
        .as_ref()
        .ok_or("Slicer executable not configured.")?;

    let mut cmd = Command::new(exe);
    cmd.arg(&args.file).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("Failed to launch slicer: {e}"))?;
    Ok(())
}
