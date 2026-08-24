//! Async Python process runner with NDJSON stdout parsing.
//!
//! Spawns `python3 printables_clone.py clone --url <URL> --dest <DIR>` and
//! streams structured progress events to the Tauri frontend via event emitter.

use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
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

/// Install Python dependencies from requirements.txt if not already present.
fn install_deps(python_bin: &PathBuf, script_dir: &PathBuf) -> Result<(), String> {
    let req_file = script_dir.join("requirements.txt");
    if !req_file.exists() {
        return Ok(()); // No requirements file, skip
    }

    // Check if deps are already installed by trying to import cloudscraper
    let check = Command::new(python_bin)
        .args(["-c", "import cloudscraper, bs4, requests"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    if let Ok(status) = check {
        if status.success() {
            return Ok(()); // All deps available
        }
    }

    // Install dependencies
    eprintln!("[python] Installing dependencies...");
    let result = Command::new(python_bin)
        .args(["-m", "pip", "install", "--user", "-r", req_file.to_str().unwrap_or("requirements.txt")])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status();

    match result {
        Ok(status) if status.success() => {
            eprintln!("[python] Dependencies installed successfully");
            Ok(())
        }
        Ok(status) => {
            eprintln!("[python] pip install failed with status: {}", status);
            Err(format!("Failed to install Python dependencies (pip exit code: {}). Make sure pip is available.", status))
        }
        Err(e) => {
            eprintln!("[python] Failed to run pip: {}", e);
            Err(format!("Failed to run pip: {e}. Make sure Python and pip are installed."))
        }
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

    // Install Python dependencies before running
    install_deps(&python_bin, &script_dir)?;

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

    // Collect stderr lines for error reporting
    let stderr_lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_lines_clone = stderr_lines.clone();

    // Stream stderr and collect lines
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(msg) = line {
                let _ = app_err.emit(
                    &format!("clone://{}/log", job_err),
                    &serde_json::json!({"level": "info", "message": msg}),
                );
                stderr_lines_clone.lock().unwrap().push(msg);
            }
        }
    });

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
                // Include stderr output in error message for debugging
                let stderr_output = stderr_lines.lock().unwrap().join("\n");
                let error_msg = if stderr_output.is_empty() {
                    format!("Python exited with status: {}", status)
                } else {
                    format!("Python exited with status: {}\n\nError output:\n{}", status, stderr_output)
                };
                let _ = app_clone.emit(
                    &format!("clone://{}/error", job_id_clone),
                    &serde_json::json!({
                        "job_id": job_id_clone,
                        "code": "EXIT_CODE",
                        "message": error_msg
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

    Ok(job_id)
}
