use sea_orm::DatabaseConnection;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use note_core::entities::blocks;
use note_core::error::AppResult;
use note_core::services::block_service::{self, BlockInput};
use crate::session::Session;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockResponse {
    pub id: Uuid,
    pub block_type: String,
    pub content: serde_json::Value,
}

impl From<blocks::Model> for BlockResponse {
    fn from(m: blocks::Model) -> Self {
        Self { id: m.id, block_type: m.block_type, content: m.content }
    }
}

#[tauri::command]
pub async fn get_document_blocks(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    document_id: Uuid,
) -> AppResult<Vec<BlockResponse>> {
    let user_id = session.require_user()?;
    let list = block_service::get_document_blocks(db.inner(), user_id, document_id).await?;
    Ok(list.into_iter().map(Into::into).collect())
}

/// Replace the page's content with `blocks` (full list, in order).
#[tauri::command]
pub async fn save_document_blocks(
    db: State<'_, DatabaseConnection>,
    session: State<'_, Session>,
    document_id: Uuid,
    blocks: Vec<BlockInput>,
) -> AppResult<()> {
    let user_id = session.require_user()?;
    block_service::save_document_blocks(db.inner(), user_id, document_id, blocks).await
}
