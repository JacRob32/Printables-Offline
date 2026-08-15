//! Async Python process runner with NDJSON stdout parsing.
//!
//! Spawns `python3 printables_clone.py clone --url <URL> --dest <DIR>` and
//! streams structured progress events to the Tauri frontend via event emitter.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Emitter};

/// One NDJSON record emitted by the Python adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PyProgress {
    pub kind: String,       // "phase" | "file_progress" | "done" | "error"
    #[serde(flatten)]
    pub payload: serde_json::Value,
}

/// Resolve the Python interpreter path: prefs override → env var → system lookup.
pub fn resolve_python(prefs_path: Option<&str>) -> PathBuf {
    if let Some(p) = prefs_path.filter(|s| !s.is_empty()) {
        return PathBuf::from(p);
    }
    if let Ok(p) = std::env::var("PRINTABLES_PYTHON") {
        return PathBuf::from(p);
    }
    // Platform-aware fallback
    if cfg!(windows) {
        PathBuf::from("py") // Windows py launcher
    } else {
        PathBuf::from("python3")
    }
}

/// Spawn the clone subprocess and stream NDJSON records as Tauri events.
/// Returns immediately with a job_id; progress is delivered via events.
pub fn spawn_clone(
    app: &AppHandle,
    python_bin: PathBuf,
    script_dir: PathBuf,
    url: String,
    dest: String,
    debug: bool,
) -> Result<String, String> {
    let job_id = uuid::Uuid::new_v4().to_string();
    let script = script_dir.join("printables_clone.py");

    if !script.exists() {
        return Err(format!("Clone script not found: {}", script.display()));
    }

    let mut cmd = Command::new(&python_bin);
    cmd.arg(&script)
        .arg("clone")
        .arg("--url")
        .arg(&url)
        .arg("--dest")
        .arg(&dest);
    if debug {
        cmd.arg("--debug");
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&script_dir);

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn Python: {e}"))?;

    let stdout = child.stdout.take().expect("stdout should be piped");
    let stderr = child.stderr.take().expect("stderr should be piped");
    let app_clone = app.clone();
    let job_id_clone = job_id.clone();
    let app_err = app.clone();
    let job_err = job_id.clone();

    // Parse stdout NDJSON on a dedicated thread
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(raw) => {
                    if let Ok(record) = serde_json::from_str::<PyProgress>(&raw) {
                        let _ = app_clone.emit(
                            &format!("clone://{}/{}/{}", job_id_clone, "progress", record.kind),
                            &record,
                        );
                    }
                }
                Err(e) => {
                    let _ = app_clone.emit(
                        &format!("clone://{}/log", job_id_clone),
                        &serde_json::json!({"level": "warn", "message": e.to_string()}),
                    );
                }
            }
        }
        // Wait for exit and emit done/error
        match child.wait() {
            Ok(status) if status.success() => {
                let _ = app_clone.emit(
                    &format!("clone://{}/done", job_id_clone),
                    &serde_json::json!({"job_id": job_id_clone}),
                );
            }
            Ok(status) => {
                let _ = app_clone.emit(
                    &format!("clone://{}/error", job_id_clone),
                    &serde_json::json!({
                        "job_id": job_id_clone,
                        "code": "EXIT_CODE",
                        "message": format!("Python exited with status: {}", status)
                    }),
                );
            }
            Err(e) => {
                let _ = app_clone.emit(
                    &format!("clone://{}/error", job_id_clone),
                    &serde_json::json!({
                        "job_id": job_id_clone,
                        "code": "PROCESS",
                        "message": e.to_string()
                    }),
                );
            }
        }
    });

    // Stream stderr separately (human-readable logs)
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(msg) = line {
                let _ = app_err.emit(
                    &format!("clone://{}/log", job_err),
                    &serde_json::json!({"level": "info", "message": msg}),
                );
            }
        }
    });

    Ok(job_id)
}
