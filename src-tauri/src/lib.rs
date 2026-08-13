mod commands;
mod library;
mod models;
mod python;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(models::AppPrefs::default())
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
        ])
        .setup(|app| {
            #[cfg(debug_assertions)]
            let _ = app.handle().plugin(tauri_plugin_devtools::init());

            // Initialize default prefs in store on first launch
            let store = app.state::<tauri_plugin_store::StoreCollection>();
            if store.get("prefs").is_none() {
                // Store will be created lazily on first set_prefs call
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
