use chrono::Utc;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use uuid::Uuid;

use crate::entities::{documents, workspaces};
use crate::error::{AppError, AppResult};
use crate::services::workspace_service::get_owned_workspace;

/// Titles may be empty: the UI shows an "Untitled" placeholder instead.
fn normalize_title(title: &str) -> AppResult<String> {
    let title = title.trim();
    if title.chars().count() > 255 {
        return Err(AppError::Invalid("Title must be at most 255 characters".into()));
    }
    Ok(title.to_string())
}

/// The document, if it exists *and* lives in a workspace owned by `user_id`.
pub async fn get_owned_document<C: ConnectionTrait>(db: &C, user_id: Uuid, id: Uuid) -> AppResult<documents::Model> {
    documents::Entity::find_by_id(id)
        .inner_join(workspaces::Entity)
        .filter(workspaces::Column::OwnerId.eq(user_id))
        .one(db)
        .await?
        .ok_or(AppError::NotFound("Document"))
}

pub async fn create_document<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    workspace_id: Uuid,
    title: &str,
    parent_id: Option<Uuid>,
) -> AppResult<documents::Model> {
    get_owned_workspace(db, user_id, workspace_id).await?;
    if let Some(parent_id) = parent_id {
        let parent = get_owned_document(db, user_id, parent_id).await?;
        if parent.workspace_id != workspace_id {
            return Err(AppError::Invalid("Parent page is in a different workspace".into()));
        }
    }

    let doc = documents::ActiveModel {
        id: Set(Uuid::new_v4()),
        title: Set(normalize_title(title)?),
        workspace_id: Set(workspace_id),
        parent_id: Set(parent_id),
        is_archived: Set(false),
        ..Default::default()
    };
    Ok(doc.insert(db).await?)
}

/// All non-archived documents in a workspace (flat; build the tree from `parent_id`).
pub async fn list_documents<C: ConnectionTrait>(db: &C, user_id: Uuid, workspace_id: Uuid) -> AppResult<Vec<documents::Model>> {
    get_owned_workspace(db, user_id, workspace_id).await?;
    Ok(documents::Entity::find()
        .filter(documents::Column::WorkspaceId.eq(workspace_id))
        .filter(documents::Column::IsArchived.eq(false))
        .order_by_asc(documents::Column::CreatedAt)
        .all(db)
        .await?)
}

pub async fn update_document_title<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    id: Uuid,
    title: &str,
) -> AppResult<documents::Model> {
    let mut active: documents::ActiveModel = get_owned_document(db, user_id, id).await?.into();
    active.title = Set(normalize_title(title)?);
    Ok(active.update(db).await?)
}

/// IDs of all descendants of `id` (children, grandchildren, ...).
async fn descendant_ids<C: ConnectionTrait>(db: &C, id: Uuid) -> AppResult<Vec<Uuid>> {
    let mut all = Vec::new();
    let mut frontier = vec![id];
    while !frontier.is_empty() {
        let children: Vec<Uuid> = documents::Entity::find()
            .select_only()
            .column(documents::Column::Id)
            .filter(documents::Column::ParentId.is_in(frontier))
            .into_tuple()
            .all(db)
            .await?;
        all.extend_from_slice(&children);
        frontier = children;
    }
    Ok(all)
}

/// Archive a page and all pages nested under it. Returns the archived IDs.
pub async fn archive_document<C: ConnectionTrait>(db: &C, user_id: Uuid, id: Uuid) -> AppResult<Vec<Uuid>> {
    get_owned_document(db, user_id, id).await?;
    let mut ids = vec![id];
    ids.extend(descendant_ids(db, id).await?);

    documents::Entity::update_many()
        .col_expr(documents::Column::IsArchived, Expr::value(true))
        .col_expr(documents::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(documents::Column::Id.is_in(ids.clone()))
        .exec(db)
        .await?;
    Ok(ids)
}

/// Move a page under another page (`Some`) or to the top level (`None`).
pub async fn move_document<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    id: Uuid,
    new_parent_id: Option<Uuid>,
) -> AppResult<documents::Model> {
    let doc = get_owned_document(db, user_id, id).await?;

    if let Some(parent_id) = new_parent_id {
        let parent = get_owned_document(db, user_id, parent_id).await?;
        if parent.workspace_id != doc.workspace_id {
            return Err(AppError::Invalid("Cannot move a page to a different workspace".into()));
        }
        // Walk up from the new parent: reaching `id` means we'd create a cycle.
        let mut cursor = Some(parent);
        while let Some(p) = cursor {
            if p.id == id {
                return Err(AppError::Invalid("Cannot move a page inside itself".into()));
            }
            cursor = match p.parent_id {
                Some(pid) => documents::Entity::find_by_id(pid).one(db).await?,
                None => None,
            };
        }
    }

    let mut active: documents::ActiveModel = doc.into();
    active.parent_id = Set(new_parent_id);
    Ok(active.update(db).await?)
}

/// Archived ("in the trash") documents of a workspace, most recently deleted first.
pub async fn list_archived_documents<C: ConnectionTrait>(
    db: &C,
    user_id: Uuid,
    workspace_id: Uuid,
) -> AppResult<Vec<documents::Model>> {
    get_owned_workspace(db, user_id, workspace_id).await?;
    Ok(documents::Entity::find()
        .filter(documents::Column::WorkspaceId.eq(workspace_id))
        .filter(documents::Column::IsArchived.eq(true))
        .order_by_desc(documents::Column::UpdatedAt)
        .all(db)
        .await?)
}

/// Restore an archived page and the pages nested under it. If its parent is
/// still in the trash, the page is restored to the top level instead.
/// Returns the restored documents.
pub async fn restore_document<C: ConnectionTrait>(db: &C, user_id: Uuid, id: Uuid) -> AppResult<Vec<documents::Model>> {
    let doc = get_owned_document(db, user_id, id).await?;

    if let Some(parent_id) = doc.parent_id {
        let parent_archived = documents::Entity::find_by_id(parent_id)
            .one(db)
            .await?
            .is_none_or(|p| p.is_archived);
        if parent_archived {
            let mut active: documents::ActiveModel = doc.into();
            active.parent_id = Set(None);
            active.update(db).await?;
        }
    }

    let mut ids = vec![id];
    ids.extend(descendant_ids(db, id).await?);
    documents::Entity::update_many()
        .col_expr(documents::Column::IsArchived, Expr::value(false))
        .col_expr(documents::Column::UpdatedAt, Expr::value(Utc::now()))
        .filter(documents::Column::Id.is_in(ids.clone()))
        .exec(db)
        .await?;

    Ok(documents::Entity::find()
        .filter(documents::Column::Id.is_in(ids))
        .order_by_asc(documents::Column::CreatedAt)
        .all(db)
        .await?)
}

/// Permanently delete a page that is in the trash, with its nested pages and
/// all their content. Returns the deleted IDs.
pub async fn delete_document_permanently<C: ConnectionTrait>(db: &C, user_id: Uuid, id: Uuid) -> AppResult<Vec<Uuid>> {
    let doc = get_owned_document(db, user_id, id).await?;
    if !doc.is_archived {
        return Err(AppError::Invalid("Move the page to the trash before deleting it permanently".into()));
    }

    // Deepest pages first, so no row is deleted while a child still points at it.
    let mut ids = descendant_ids(db, id).await?;
    ids.reverse();
    ids.push(id);
    for chunk in ids.chunks(500) {
        documents::Entity::delete_many()
            .filter(documents::Column::Id.is_in(chunk.to_vec()))
            .exec(db)
            .await?;
    }
    Ok(ids)
}
