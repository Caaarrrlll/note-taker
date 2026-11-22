use tauri::State;

use note_core::api::UserResponse;
use note_core::error::AppResult;
use note_core::services::auth_service;
use crate::spaces::Spaces;


/// The space's user (the local user of its database).
#[tauri::command]
pub async fn get_current_user(spaces: State<'_, Spaces>, space_id: String) -> AppResult<UserResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(auth_service::get_user(&space.db, space.user_id).await?.into())
}
