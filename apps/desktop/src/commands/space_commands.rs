use std::path::PathBuf;

use tauri::State;

use note_core::error::AppResult;
use crate::spaces::{Space, Spaces};

#[tauri::command]
pub fn list_spaces(spaces: State<'_, Spaces>) -> Vec<Space> {
    spaces.list()
}

/// `path`: an existing notes file to open, or a new file to create. Omit it to
/// put the file in the app data folder.
#[tauri::command]
pub async fn create_space(
    spaces: State<'_, Spaces>,
    name: String,
    color: String,
    path: Option<String>,
) -> AppResult<Space> {
    spaces.create(&name, &color, path.map(PathBuf::from)).await
}

#[tauri::command]
pub fn update_space(spaces: State<'_, Spaces>, id: String, name: String, color: String) -> AppResult<Space> {
    spaces.update(&id, &name, &color)
}

/// Removes the space from the list; its file stays on disk.
#[tauri::command]
pub async fn remove_space(spaces: State<'_, Spaces>, id: String) -> AppResult<()> {
    spaces.remove(&id).await
}

/// Copies the space's file to `path`, verifies it, and switches to it. The old file is kept.
#[tauri::command]
pub async fn move_space_file(spaces: State<'_, Spaces>, id: String, path: String) -> AppResult<Space> {
    spaces.move_file(&id, PathBuf::from(path)).await
}
