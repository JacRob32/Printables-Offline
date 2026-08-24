use serde::Deserialize;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use tauri::State;
use crate::models::AppPrefs;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenInSlicerArgs {
    pub model_id: String,
    pub slicer: String,
    pub files: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceFileArgs {
    pub file: String,
}

/// On macOS, resolve a binary path inside a .app bundle to the .app path
/// so we can launch it via `open -a` (bypasses sandbox restrictions).
fn resolve_app_bundle(exe: &str) -> Option<String> {
    let p = Path::new(exe);
    let mut current = p;
    while let Some(parent) = current.parent() {
        if parent.extension().map(|e| e == "app").unwrap_or(false) {
            return Some(parent.display().to_string());
        }
        current = parent;
    }
    None
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

    // On macOS, prefer `open -a <app>` to bypass sandbox
    if cfg!(target_os = "macos") {
        if let Some(app_path) = resolve_app_bundle(exe) {
            let mut cmd = Command::new("open");
            cmd.arg("-a").arg(&app_path);
            for f in &args.files {
                cmd.arg(f);
            }
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            cmd.spawn().map_err(|e| format!("Failed to launch slicer: {e}"))?;
            return Ok(());
        }
    }

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

    // On macOS, prefer `open -a <app>` to bypass sandbox
    if cfg!(target_os = "macos") {
        if let Some(app_path) = resolve_app_bundle(exe) {
            let mut cmd = Command::new("open");
            cmd.arg("-a").arg(&app_path).arg(&args.file);
            cmd.stdout(Stdio::null()).stderr(Stdio::null());
            cmd.spawn().map_err(|e| format!("Failed to launch slicer: {e}"))?;
            return Ok(());
        }
    }

    let mut cmd = Command::new(exe);
    cmd.arg(&args.file).stdout(Stdio::null()).stderr(Stdio::null());
    cmd.spawn().map_err(|e| format!("Failed to launch slicer: {e}"))?;
    Ok(())
}
