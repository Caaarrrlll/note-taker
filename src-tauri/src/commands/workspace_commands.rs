use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services;

#[derive(Serialize, Deserialize)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub owner_id: Uuid,
}

impl From<crate::entities::workspaces::Model> for WorkspaceResponse {
    fn from(m: crate::entities::workspaces::Model) -> Self {
        Self {
            id: m.id,
            name: m.name,
            owner_id: m.owner_id,
        }
    }
}

#[tauri::command]
pub async fn create_workspace(owner_id: Uuid, name: String) -> Result<WorkspaceResponse, String> {
    services::create_workspace(owner_id, &name)
        .await
        .map(WorkspaceResponse::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_workspaces(owner_id: Uuid) -> Result<Vec<WorkspaceResponse>, String> {
    services::list_user_workspaces(owner_id)
        .await
        .map(|ws| ws.into_iter().map(WorkspaceResponse::from).collect())
        .map_err(|e| e.to_string())
}
