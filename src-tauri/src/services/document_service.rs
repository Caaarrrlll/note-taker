use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::db::get_db;
use crate::entities::{documents, blocks};

pub async fn create_document(workspace_id: Uuid, title: &str, parent_id: Option<Uuid>) -> Result<documents::Model, DbErr> {
    let db = get_db();
    let doc = documents::ActiveModel {
        id: Set(Uuid::new_v4()),
        title: Set(title.to_string()),
        workspace_id: Set(workspace_id),
        parent_id: Set(parent_id),
        is_archived: Set(false),
        ..Default::default()
    };
    doc.insert(db).await
}

pub async fn get_document(id: Uuid) -> Result<Option<documents::Model>, DbErr> {
    let db = get_db();
    documents::Entity::find_by_id(id).one(db).await
}

pub async fn list_documents(workspace_id: Uuid) -> Result<Vec<documents::Model>, DbErr> {
    let db = get_db();
    documents::Entity::find()
        .filter(documents::Column::WorkspaceId.eq(workspace_id))
        .filter(documents::Column::IsArchived.eq(false))
        .all(db)
        .await
}

pub async fn update_document_title(id: Uuid, title: &str) -> Result<documents::Model, DbErr> {
    let db = get_db();
    let doc = documents::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Document not found".into()))?;

    let mut active: documents::ActiveModel = doc.into();
    active.title = Set(title.to_string());
    active.update(db).await
}

pub async fn archive_document(id: Uuid) -> Result<documents::Model, DbErr> {
    let db = get_db();
    let doc = documents::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(DbErr::RecordNotFound("Document not found".into()))?;

    let mut active: documents::ActiveModel = doc.into();
    active.is_archived = Set(true);
    active.update(db).await
}

pub async fn get_document_blocks(document_id: Uuid) -> Result<Vec<blocks::Model>, DbErr> {
    let db = get_db();
    blocks::Entity::find()
        .filter(blocks::Column::DocumentId.eq(document_id))
        .all(db)
        .await
}
