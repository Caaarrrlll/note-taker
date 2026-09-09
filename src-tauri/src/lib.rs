mod db;
mod entities;
mod services;
mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            commands::create_document,
            commands::get_document,
            commands::list_documents,
            commands::update_document_title,
            commands::archive_document,
            commands::create_workspace,
            commands::list_workspaces,
        ])
        .setup(|_app| {
            let database_url = std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/note_taker".into());

            tauri::async_runtime::block_on(async {
                db::init(&database_url)
                    .await
                    .expect("Failed to initialize database");
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
