use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use note_core::entities::users;
use note_core::error::AppResult;
use note_core::services::auth_service;
use crate::spaces::Spaces;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_local: bool,
}

impl From<users::Model> for UserResponse {
    fn from(m: users::Model) -> Self {
        Self { id: m.id, email: m.email, display_name: m.display_name, is_local: m.is_local }
    }
}

/// The space's user (the local user of its database).
#[tauri::command]
pub async fn get_current_user(spaces: State<'_, Spaces>, space_id: String) -> AppResult<UserResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(auth_service::get_user(&space.db, space.user_id).await?.into())
}
