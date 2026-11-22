use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use note_core::entities::documents;
use note_core::error::AppResult;
use note_core::services::document_service;
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub id: Uuid,
    pub title: String,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<documents::Model> for DocumentResponse {
    fn from(m: documents::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            workspace_id: m.workspace_id,
            parent_id: m.parent_id,
            is_archived: m.is_archived,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[tauri::command]
pub async fn list_documents(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    workspace_id: Uuid,
) -> AppResult<Vec<DocumentResponse>> {
    let user_id = session.require_user()?;
    let docs = document_service::list_documents(db.inner(), user_id, workspace_id).await?;
    Ok(docs.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_document(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    id: Uuid,
) -> AppResult<DocumentResponse> {
    let user_id = session.require_user()?;
    Ok(document_service::get_owned_document(db.inner(), user_id, id).await?.into())
}

#[tauri::command]
pub async fn create_document(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    workspace_id: Uuid,
    title: String,
    parent_id: Option<Uuid>,
) -> AppResult<DocumentResponse> {
    let user_id = session.require_user()?;
    Ok(document_service::create_document(db.inner(), user_id, workspace_id, &title, parent_id).await?.into())
}

#[tauri::command]
pub async fn update_document_title(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    id: Uuid,
    title: String,
) -> AppResult<DocumentResponse> {
    let user_id = session.require_user()?;
    Ok(document_service::update_document_title(db.inner(), user_id, id, &title).await?.into())
}

/// Archives the page and everything nested under it; returns the archived IDs.
#[tauri::command]
pub async fn archive_document(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let user_id = session.require_user()?;
    document_service::archive_document(db.inner(), user_id, id).await
}

#[tauri::command]
pub async fn move_document(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    id: Uuid,
    parent_id: Option<Uuid>,
) -> AppResult<DocumentResponse> {
    let user_id = session.require_user()?;
    Ok(document_service::move_document(db.inner(), user_id, id, parent_id).await?.into())
}

/// Pages in the trash (archived), most recently deleted first.
#[tauri::command]
pub async fn list_archived_documents(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    workspace_id: Uuid,
) -> AppResult<Vec<DocumentResponse>> {
    let user_id = session.require_user()?;
    let docs = document_service::list_archived_documents(db.inner(), user_id, workspace_id).await?;
    Ok(docs.into_iter().map(Into::into).collect())
}

/// Restores the page and its nested pages; returns the restored pages.
#[tauri::command]
pub async fn restore_document(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    id: Uuid,
) -> AppResult<Vec<DocumentResponse>> {
    let user_id = session.require_user()?;
    let docs = document_service::restore_document(db.inner(), user_id, id).await?;
    Ok(docs.into_iter().map(Into::into).collect())
}

/// Permanently deletes a trashed page and its nested pages; returns the deleted IDs.
#[tauri::command]
pub async fn delete_document_permanently(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let user_id = session.require_user()?;
    document_service::delete_document_permanently(db.inner(), user_id, id).await
}
