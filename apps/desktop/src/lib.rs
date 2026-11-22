mod commands;
mod spaces;

use tauri::Manager;

use note_core::db;
use crate::spaces::Spaces;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_spaces,
            commands::create_space,
            commands::update_space,
            commands::remove_space,
            commands::move_space_file,
            commands::get_current_user,
            commands::list_workspaces,
            commands::create_workspace,
            commands::list_documents,
            commands::get_document,
            commands::create_document,
            commands::update_document_title,
            commands::archive_document,
            commands::move_document,
            commands::list_archived_documents,
            commands::restore_document,
            commands::delete_document_permanently,
            commands::get_document_blocks,
            commands::save_document_blocks,
        ])
        .setup(|app| {
            // Load .env from the current dir or any parent (repo root works from apps/desktop/).
            // Real environment variables take precedence over the file.
            let _ = dotenvy::dotenv();

            let data_dir = app.path().app_data_dir()?;
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&data_dir)?;

            // DATABASE_URL (dev/CI) overrides the first space's storage.
            let first_space_override = match std::env::var("DATABASE_URL") {
                Ok(url) if !url.trim().is_empty() => Some(db::DbConfig::from_env(&data_dir)?),
                _ => None,
            };
            if let Some(config) = &first_space_override {
                eprintln!("First space uses DATABASE_URL ({})", db::backend_name(&config.url));
            }

            let spaces = Spaces::load(config_dir.join("spaces.json"), data_dir, first_space_override)?;
            eprintln!("{} space(s) in {}", spaces.list().len(), config_dir.join("spaces.json").display());
            app.manage(spaces);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
