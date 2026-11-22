use tauri::State;
use uuid::Uuid;

use note_core::api::BlockResponse;
use note_core::error::AppResult;
use note_core::services::block_service::{self, BlockInput};
use crate::spaces::Spaces;


#[tauri::command]
pub async fn get_document_blocks(
    spaces: State<'_, Spaces>,
    space_id: String,
    document_id: Uuid,
) -> AppResult<Vec<BlockResponse>> {
    let space = spaces.open(&space_id).await?;
    let list = block_service::get_document_blocks(&space.db, space.user_id, document_id).await?;
    Ok(list.into_iter().map(Into::into).collect())
}

/// Replace the page's content with `blocks` (full list, in order).
#[tauri::command]
pub async fn save_document_blocks(
    spaces: State<'_, Spaces>,
    space_id: String,
    document_id: Uuid,
    blocks: Vec<BlockInput>,
) -> AppResult<()> {
    let space = spaces.open(&space_id).await?;
    block_service::save_document_blocks(&space.db, space.user_id, document_id, blocks).await
}
