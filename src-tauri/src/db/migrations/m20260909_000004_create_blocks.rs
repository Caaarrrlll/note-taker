use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Blocks::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Blocks::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Blocks::DocumentId).uuid().not_null())
                    .col(ColumnDef::new(Blocks::SortOrder).integer().not_null())
                    .col(ColumnDef::new(Blocks::BlockType).string().not_null())
                    .col(ColumnDef::new(Blocks::Content).json_binary().not_null().default(Expr::cust("'{}'::jsonb")))
                    .col(ColumnDef::new(Blocks::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Blocks::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_block_document")
                            .from(Blocks::Table, Blocks::DocumentId)
                            .to(Documents::Table, Documents::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Blocks::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Blocks {
    Table,
    Id,
    DocumentId,
    SortOrder,
    BlockType,
    Content,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Documents {
    Table,
    Id,
}
