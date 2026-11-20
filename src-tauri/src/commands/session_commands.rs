use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::entities::users;
use crate::error::AppResult;
use crate::services::auth_service;
use crate::session::Session;

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

/// The signed-in user. Fails with kind `unauthenticated` when nobody is signed in.
#[tauri::command]
pub async fn get_current_user(db: State<'_, DatabaseConnection>, session: State<'_, Session>) -> AppResult<UserResponse> {
    let user_id = session.require_user()?;
    Ok(auth_service::get_user(db.inner(), user_id).await?.into())
}
