use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::get_db;
use crate::entities::{workspaces, documents};

pub async fn create_workspace(owner_id: Uuid, name: &str) -> Result<workspaces::Model, DbErr> {
    let db = get_db();
    let ws = workspaces::ActiveModel {
        id: Set(Uuid::new_v4()),
        name: Set(name.to_string()),
        owner_id: Set(owner_id),
        ..Default::default()
    };
    ws.insert(db).await
}

pub async fn get_workspace(id: Uuid) -> Result<Option<workspaces::Model>, DbErr> {
    let db = get_db();
    workspaces::Entity::find_by_id(id).one(db).await
}

pub async fn list_user_workspaces(owner_id: Uuid) -> Result<Vec<workspaces::Model>, DbErr> {
    let db = get_db();
    workspaces::Entity::find()
        .filter(workspaces::Column::OwnerId.eq(owner_id))
        .all(db)
        .await
}

pub async fn get_workspace_documents(workspace_id: Uuid) -> Result<Vec<documents::Model>, DbErr> {
    let db = get_db();
    documents::Entity::find()
        .filter(documents::Column::WorkspaceId.eq(workspace_id))
        .filter(documents::Column::ParentId.is_null())
        .filter(documents::Column::IsArchived.eq(false))
        .all(db)
        .await
}
