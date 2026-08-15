/// Rebuild thumbnails — placeholder. Real implementation would regenerate
/// preview renders for every model using a headless 3D renderer.
#[tauri::command]
pub fn rebuild_thumbs() -> Result<String, String> {
    // TODO: integrate a headless 3D renderer (e.g., wgpu + gltf) to produce
    // ISO-view PNG thumbnails for models without cover images.
    Ok("Thumbnail rebuild queued (not yet implemented)".into())
}
