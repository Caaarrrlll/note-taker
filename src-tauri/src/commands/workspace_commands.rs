use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use crate::entities::workspaces;
use crate::error::AppResult;
use crate::services::workspace_service;
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
}

impl From<workspaces::Model> for WorkspaceResponse {
    fn from(m: workspaces::Model) -> Self {
        Self { id: m.id, name: m.name }
    }
}

#[tauri::command]
pub async fn list_workspaces(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
) -> AppResult<Vec<WorkspaceResponse>> {
    let user_id = session.require_user()?;
    let list = workspace_service::list_user_workspaces(db.inner(), user_id).await?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn create_workspace(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    name: String,
) -> AppResult<WorkspaceResponse> {
    let user_id = session.require_user()?;
    Ok(workspace_service::create_workspace(db.inner(), user_id, &name).await?.into())
}
