use tauri::State;
use crate::library::indexer;
use crate::models::{AppPrefs, LibraryIndex};

/// Rebuild thumbnails — placeholder. Real implementation would regenerate
/// preview renders for every model using a 3D viewport renderer.
#[tauri::command]
pub fn rebuild_thumbs() -> Result<String, String> {
    // TODO: integrate a headless 3D renderer (e.g., wgpu + gltf) to produce
    // ISO-view PNG thumbnails for models without cover images.
    Ok("Thumbnail rebuild queued (not yet implemented)".into())
}

/// Rescan library and return updated index.
#[tauri::command]
pub fn rescan_library(prefs: State<'_, AppPrefs>) -> Result<LibraryIndex, String> {
    let lib = prefs
        .library_folder
        .as_ref()
        .ok_or("Library folder not configured.")?;
    indexer::index_library(std::path::Path::new(lib))
}
