use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, QueryOrder, Set};
use uuid::Uuid;

use crate::entities::workspaces;
use crate::error::{AppError, AppResult};

pub const DEFAULT_WORKSPACE_NAME: &str = "My Notes";

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 255 {
        return Err(AppError::Invalid("Workspace name must be 1–255 characters".into()));
    }
    Ok(name.to_string())
}

pub async fn create_workspace<C: ConnectionTrait>(db: &C, owner_id: Uuid, name: &str) -> AppResult<workspaces::Model> {
    let ws = workspaces::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(validate_name(name)?),
        owner_id: Set(owner_id),
        ..Default::default()
    };
    Ok(ws.insert(db).await?)
}

pub async fn list_user_workspaces<C: ConnectionTrait>(db: &C, owner_id: Uuid) -> AppResult<Vec<workspaces::Model>> {
    Ok(workspaces::Entity::find()
        .filter(workspaces::Column::OwnerId.eq(owner_id))
        .order_by_asc(workspaces::Column::CreatedAt)
        .all(db)
        .await?)
}

/// The workspace, if it exists *and* belongs to `owner_id`; otherwise NotFound.
pub async fn get_owned_workspace<C: ConnectionTrait>(db: &C, owner_id: Uuid, id: Uuid) -> AppResult<workspaces::Model> {
    workspaces::Entity::find_by_id(id)
        .filter(workspaces::Column::OwnerId.eq(owner_id))
        .one(db)
        .await?
        .ok_or(AppError::NotFound("Workspace"))
}

/// Make sure the user has at least one workspace (first run).
pub async fn ensure_default_workspace<C: ConnectionTrait>(db: &C, owner_id: Uuid) -> AppResult<workspaces::Model> {
    match list_user_workspaces(db, owner_id).await?.into_iter().next() {
        Some(ws) => Ok(ws),
        None => create_workspace(db, owner_id, DEFAULT_WORKSPACE_NAME).await,
    }
}
