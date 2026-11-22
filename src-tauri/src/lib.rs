mod commands;
mod session;

use tauri::Manager;

use note_core::db;
use note_core::services::{auth_service, workspace_service};
use crate::session::Session;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
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
            // Load .env from the current dir or any parent (repo root works from src-tauri/).
            // Real environment variables take precedence over the file.
            let _ = dotenvy::dotenv();

            // Without DATABASE_URL, fall back to a local SQLite file in the app data dir.
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let config = db::DbConfig::from_env(&data_dir)?;
            let backend = db::backend_name(&config.url);

            let (conn, user) = tauri::async_runtime::block_on(async {
                let conn = db::connect(&config)
                    .await
                    .map_err(|e| format!("Failed to initialize {backend} database: {e}"))?;

                // Local mode: sign in the single local user, creating it (and a
                // first workspace) on first run. A login screen would replace this.
                let user = auth_service::ensure_local_user(&conn).await.map_err(|e| e.to_string())?;
                workspace_service::ensure_default_workspace(&conn, user.id)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<_, String>((conn, user))
            })?;

            eprintln!("Using {backend} database");
            if config.url.starts_with("sqlite") {
                eprintln!("  {}", config.url);
            }

            let session = Session::default();
            session.sign_in(user.id);
            app.manage(session);
            app.manage(conn);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
