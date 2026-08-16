mod commands;
mod library;
mod models;
mod python;

use std::sync::Mutex;
use tauri::Manager;
use tauri_plugin_store::StoreExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(Mutex::new(models::AppPrefs::default()))
        .invoke_handler(tauri::generate_handler![
            commands::clone::clone_model,
            commands::library::list_models,
            commands::library::library_stats,
            commands::library::rescan_library,
            commands::prefs::get_prefs,
            commands::prefs::set_prefs,
            commands::dialogs::dialog_open,
            commands::slicer::open_in_slicer,
            commands::slicer::slice_file,
            commands::maintenance::rebuild_thumbs,
            commands::export::export_files,
            commands::shell::open_folder,
            commands::shell::open_external,
        ])
        .setup(|app| {
            // Hydrate managed AppPrefs state from the store on startup
            let store = app.store("prefs.json").map_err(|e| e.to_string())?;
            let prefs: models::AppPrefs = store
                .get("app_prefs")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();

            // Update managed state
            let managed = app.state::<Mutex<models::AppPrefs>>();
            let mut managed_prefs = managed.lock().map_err(|e| e.to_string())?;
            *managed_prefs = prefs.clone();

            // Widen asset scope for library folder if configured
            if let Some(lib_path) = &prefs.library_folder {
                if let Ok(path) = std::path::PathBuf::from(lib_path).canonicalize() {
                    let _ = app.asset_protocol_scope().allow_directory(&path, true);
                }
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
