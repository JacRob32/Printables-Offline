mod commands;
mod library;
mod models;
mod python;

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
        .setup(|_app| {
            // Runtime asset scope: the library folder is added dynamically
            // inside set_prefs when the user selects/changes it.
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
