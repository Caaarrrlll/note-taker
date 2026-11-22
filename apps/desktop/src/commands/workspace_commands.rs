use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use note_core::entities::workspaces;
use note_core::error::AppResult;
use note_core::services::workspace_service;
use crate::spaces::Spaces;

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
    spaces: State<'_, Spaces>,
    space_id: String,
) -> AppResult<Vec<WorkspaceResponse>> {
    let space = spaces.open(&space_id).await?;
    let list = workspace_service::list_user_workspaces(&space.db, space.user_id).await?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn create_workspace(
    spaces: State<'_, Spaces>,
    space_id: String,
    name: String,
) -> AppResult<WorkspaceResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(workspace_service::create_workspace(&space.db, space.user_id, &name).await?.into())
}
