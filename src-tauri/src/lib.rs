use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .setup(|app| {
            // Runtime asset scope: allow the library folder (resolved at runtime)
            // This will be extended when the user selects a library folder in Stage 4+.
            #[cfg(debug_assertions)]
            let _ = app.handle().plugin(tauri_plugin_devtools::init());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
