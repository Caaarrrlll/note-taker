use tauri::State;
use uuid::Uuid;

use note_core::api::DocumentResponse;
use note_core::error::AppResult;
use note_core::services::document_service;
use crate::spaces::Spaces;


#[tauri::command]
pub async fn list_documents(
    spaces: State<'_, Spaces>,
    space_id: String,
    workspace_id: Uuid,
) -> AppResult<Vec<DocumentResponse>> {
    let space = spaces.open(&space_id).await?;
    let docs = document_service::list_documents(&space.db, space.user_id, workspace_id).await?;
    Ok(docs.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn get_document(
    spaces: State<'_, Spaces>,
    space_id: String,
    id: Uuid,
) -> AppResult<DocumentResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(document_service::get_owned_document(&space.db, space.user_id, id).await?.into())
}

#[tauri::command]
pub async fn create_document(
    spaces: State<'_, Spaces>,
    space_id: String,
    workspace_id: Uuid,
    title: String,
    parent_id: Option<Uuid>,
) -> AppResult<DocumentResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(document_service::create_document(&space.db, space.user_id, workspace_id, &title, parent_id).await?.into())
}

#[tauri::command]
pub async fn update_document_title(
    spaces: State<'_, Spaces>,
    space_id: String,
    id: Uuid,
    title: String,
) -> AppResult<DocumentResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(document_service::update_document_title(&space.db, space.user_id, id, &title).await?.into())
}

/// Archives the page and everything nested under it; returns the archived IDs.
#[tauri::command]
pub async fn archive_document(
    spaces: State<'_, Spaces>,
    space_id: String,
    id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let space = spaces.open(&space_id).await?;
    document_service::archive_document(&space.db, space.user_id, id).await
}

#[tauri::command]
pub async fn move_document(
    spaces: State<'_, Spaces>,
    space_id: String,
    id: Uuid,
    parent_id: Option<Uuid>,
) -> AppResult<DocumentResponse> {
    let space = spaces.open(&space_id).await?;
    Ok(document_service::move_document(&space.db, space.user_id, id, parent_id).await?.into())
}

/// Pages in the trash (archived), most recently deleted first.
#[tauri::command]
pub async fn list_archived_documents(
    spaces: State<'_, Spaces>,
    space_id: String,
    workspace_id: Uuid,
) -> AppResult<Vec<DocumentResponse>> {
    let space = spaces.open(&space_id).await?;
    let docs = document_service::list_archived_documents(&space.db, space.user_id, workspace_id).await?;
    Ok(docs.into_iter().map(Into::into).collect())
}

/// Restores the page and its nested pages; returns the restored pages.
#[tauri::command]
pub async fn restore_document(
    spaces: State<'_, Spaces>,
    space_id: String,
    id: Uuid,
) -> AppResult<Vec<DocumentResponse>> {
    let space = spaces.open(&space_id).await?;
    let docs = document_service::restore_document(&space.db, space.user_id, id).await?;
    Ok(docs.into_iter().map(Into::into).collect())
}

/// Permanently deletes a trashed page and its nested pages; returns the deleted IDs.
#[tauri::command]
pub async fn delete_document_permanently(
    spaces: State<'_, Spaces>,
    space_id: String,
    id: Uuid,
) -> AppResult<Vec<Uuid>> {
    let space = spaces.open(&space_id).await?;
    document_service::delete_document_permanently(&space.db, space.user_id, id).await
}
