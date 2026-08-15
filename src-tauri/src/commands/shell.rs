use serde::Deserialize;
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Deserialize)]
pub struct OpenFolderArgs {
    pub path: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenExternalArgs {
    pub url: String,
}

/// Reveal a local path in Finder / Explorer / file manager.
#[tauri::command]
pub fn open_folder(app: tauri::AppHandle, args: OpenFolderArgs) -> Result<(), String> {
    let expanded = if let Some(rest) = args.path.strip_prefix("~/") {
        dirs_home()
            .map(|h| h.join(rest).display().to_string())
            .unwrap_or_else(|| args.path.clone())
    } else {
        args.path.clone()
    };

    let path = std::path::Path::new(&expanded);
    if !path.exists() {
        return Err(format!("Path does not exist: {expanded}"));
    }

    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| format!("Failed to reveal folder: {e}"))
}

/// Open a URL in the default browser.
#[tauri::command]
pub fn open_external(app: tauri::AppHandle, args: OpenExternalArgs) -> Result<(), String> {
    app.opener()
        .open_url(&args.url, None::<&str>)
        .map_err(|e| format!("Failed to open URL: {e}"))
}

fn dirs_home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}
