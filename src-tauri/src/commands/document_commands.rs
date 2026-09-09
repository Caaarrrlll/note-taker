use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::services;

#[derive(Serialize, Deserialize)]
pub struct DocumentResponse {
    pub id: Uuid,
    pub title: String,
    pub workspace_id: Uuid,
    pub is_archived: bool,
}

impl From<crate::entities::documents::Model> for DocumentResponse {
    fn from(m: crate::entities::documents::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            workspace_id: m.workspace_id,
            is_archived: m.is_archived,
        }
    }
}

#[tauri::command]
pub async fn create_document(
    workspace_id: Uuid,
    title: String,
    parent_id: Option<Uuid>,
) -> Result<DocumentResponse, String> {
    services::create_document(workspace_id, &title, parent_id)
        .await
        .map(DocumentResponse::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_document(id: Uuid) -> Result<DocumentResponse, String> {
    services::get_document(id)
        .await
        .ok()
        .flatten()
        .map(DocumentResponse::from)
        .ok_or_else(|| "Document not found".to_string())
}

#[tauri::command]
pub async fn list_documents(workspace_id: Uuid) -> Result<Vec<DocumentResponse>, String> {
    services::list_documents(workspace_id)
        .await
        .map(|docs| docs.into_iter().map(DocumentResponse::from).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_document_title(id: Uuid, title: String) -> Result<DocumentResponse, String> {
    services::update_document_title(id, &title)
        .await
        .map(DocumentResponse::from)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn archive_document(id: Uuid) -> Result<DocumentResponse, String> {
    services::archive_document(id)
        .await
        .map(DocumentResponse::from)
        .map_err(|e| e.to_string())
}
