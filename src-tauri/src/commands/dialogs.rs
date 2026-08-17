use serde::Deserialize;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Deserialize)]
pub struct DialogArgs {
    pub kind: String, // "folder" | "file"
}

/// Open a native OS dialog. Returns the selected path string or null.
/// `FilePath` implements Display, so `.to_string()` handles Path and Url variants.
#[tauri::command]
pub async fn dialog_open(
    app: tauri::AppHandle,
    args: DialogArgs,
) -> Result<Option<String>, String> {
    let (tx, rx) = std::sync::mpsc::channel();

    match args.kind.as_str() {
        "folder" => {
            app.dialog().file().pick_folder(move |path| {
                let _ = tx.send(path.map(|p| p.to_string()));
            });
        }
        "file" => {
            app.dialog().file().pick_file(move |path| {
                let _ = tx.send(path.map(|p| p.to_string()));
            });
        }
        other => return Err(format!("Unknown dialog kind: {other}")),
    }

    // Wait for the dialog callback (blocks the async task, not the main thread)
    let result = rx.recv().map_err(|e| e.to_string())?;
    Ok(result)
}
