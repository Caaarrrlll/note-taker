//! Page content.
//!
//! The editor sends the page's full list of top-level blocks on every save.
//! Each block is stored as one row; `content` holds the editor's JSON for the
//! block (including nested children). Saving diffs against what is stored:
//! unchanged rows are left alone, so `created_at` survives and writes stay small.

use std::collections::{HashMap, HashSet};

use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::entities::{blocks, documents};
use crate::error::{AppError, AppResult};
use crate::services::document_service::get_owned_document;

/// Rows per INSERT; keeps statements under SQLite's bound-parameter limit.
const INSERT_CHUNK: usize = 500;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockInput {
    /// The editor's block ID. Reused as the row ID when it is a valid, unused UUID.
    pub id: Option<String>,
    pub block_type: String,
    pub content: serde_json::Value,
}

pub async fn get_document_blocks<C: ConnectionTrait>(db: &C, user_id: Uuid, document_id: Uuid) -> AppResult<Vec<blocks::Model>> {
    get_owned_document(db, user_id, document_id).await?;
    Ok(blocks::Entity::find()
        .filter(blocks::Column::DocumentId.eq(document_id))
        .order_by_asc(blocks::Column::SortOrder)
        .all(db)
        .await?)
}

/// Replace a page's content with `input` (in order), in one transaction.
pub async fn save_document_blocks<C: TransactionTrait>(
    db: &C,
    user_id: Uuid,
    document_id: Uuid,
    input: Vec<BlockInput>,
) -> AppResult<()> {
    for b in &input {
        if b.block_type.trim().is_empty() || b.block_type.chars().count() > 64 {
            return Err(AppError::Invalid("Block type must be 1–64 characters".into()));
        }
    }

    let txn = db.begin().await?;
    get_owned_document(&txn, user_id, document_id).await?;

    let existing: HashMap<Uuid, blocks::Model> = blocks::Entity::find()
        .filter(blocks::Column::DocumentId.eq(document_id))
        .all(&txn)
        .await?
        .into_iter()
        .map(|b| (b.id, b))
        .collect();

    // IDs the client sent that already belong to blocks of *other* documents
    // (e.g. content copied between pages) can't be reused as primary keys.
    let foreign_candidates: Vec<Uuid> = input
        .iter()
        .filter_map(|b| b.id.as_deref().and_then(|s| Uuid::parse_str(s).ok()))
        .filter(|id| !existing.contains_key(id))
        .collect();
    let taken: HashSet<Uuid> = if foreign_candidates.is_empty() {
        HashSet::new()
    } else {
        blocks::Entity::find()
            .filter(blocks::Column::Id.is_in(foreign_candidates))
            .all(&txn)
            .await?
            .into_iter()
            .map(|b| b.id)
            .collect()
    };

    let now = Utc::now();
    let mut keep = HashSet::new();
    let mut to_insert = Vec::new();
    let mut changed = false;

    for (index, block) in input.into_iter().enumerate() {
        let sort_order = i32::try_from(index).map_err(|_| AppError::Invalid("Too many blocks".into()))?;
        let id = match block.id.as_deref().and_then(|s| Uuid::parse_str(s).ok()) {
            Some(id) if !keep.contains(&id) && !taken.contains(&id) => id,
            _ => Uuid::new_v4(),
        };
        keep.insert(id);

        match existing.get(&id) {
            Some(old) => {
                if old.sort_order != sort_order || old.block_type != block.block_type || old.content != block.content {
                    let mut active: blocks::ActiveModel = old.clone().into();
                    active.sort_order = Set(sort_order);
                    active.block_type = Set(block.block_type);
                    active.content = Set(block.content);
                    active.update(&txn).await?;
                    changed = true;
                }
            }
            None => {
                to_insert.push(blocks::ActiveModel {
                    id: Set(id),
                    document_id: Set(document_id),
                    sort_order: Set(sort_order),
                    block_type: Set(block.block_type),
                    content: Set(block.content),
                    // insert_many bypasses before_save, so set timestamps here.
                    created_at: Set(now),
                    updated_at: Set(now),
                });
            }
        }
    }

    let removed: Vec<Uuid> = existing.keys().filter(|id| !keep.contains(id)).copied().collect();
    if !removed.is_empty() {
        blocks::Entity::delete_many()
            .filter(blocks::Column::Id.is_in(removed))
            .exec(&txn)
            .await?;
        changed = true;
    }

    if !to_insert.is_empty() {
        changed = true;
        let mut rows = to_insert.into_iter().peekable();
        while rows.peek().is_some() {
            let chunk: Vec<_> = rows.by_ref().take(INSERT_CHUNK).collect();
            blocks::Entity::insert_many(chunk).exec_without_returning(&txn).await?;
        }
    }

    if changed {
        documents::Entity::update_many()
            .col_expr(documents::Column::UpdatedAt, Expr::value(now))
            .filter(documents::Column::Id.eq(document_id))
            .exec(&txn)
            .await?;
    }

    txn.commit().await?;
    Ok(())
}
