use serde::Deserialize;
use tauri_plugin_dialog::DialogExt;

#[derive(Debug, Deserialize)]
pub struct DialogArgs {
    pub kind: String, // "folder" | "file"
}

/// Open a native OS dialog. Returns the selected path string or null.
/// `FilePath` implements Display, so `.to_string()` handles Path and Url variants.
#[tauri::command]
pub fn dialog_open(
    app: tauri::AppHandle,
    args: DialogArgs,
) -> Result<Option<String>, String> {
    match args.kind.as_str() {
        "folder" => {
            let result = app.dialog().file().blocking_pick_folder();
            Ok(result.map(|p| p.to_string()))
        }
        "file" => {
            let result = app.dialog().file().blocking_pick_file();
            Ok(result.map(|p| p.to_string()))
        }
        other => Err(format!("Unknown dialog kind: {other}")),
    }
}
