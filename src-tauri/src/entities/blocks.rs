use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "blocks")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub document_id: Uuid,
    pub sort_order: i32,
    pub block_type: String,
    pub content: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::documents::Entity",
        from = "Column::DocumentId",
        to = "super::documents::Column::Id"
    )]
    Document,
}

impl Related<super::documents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Document.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
