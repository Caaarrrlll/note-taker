use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Documents::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Documents::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Documents::Title).string().not_null().default("Untitled"))
                    .col(ColumnDef::new(Documents::WorkspaceId).uuid().not_null())
                    .col(ColumnDef::new(Documents::ParentId).uuid().null())
                    .col(ColumnDef::new(Documents::IsArchived).boolean().not_null().default(false))
                    .col(ColumnDef::new(Documents::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Documents::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_workspace")
                            .from(Documents::Table, Documents::WorkspaceId)
                            .to(Workspaces::Table, Workspaces::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_document_parent")
                            .from(Documents::Table, Documents::ParentId)
                            .to(Documents::Table, Documents::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Documents::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Documents {
    Table,
    Id,
    Title,
    WorkspaceId,
    ParentId,
    IsArchived,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Workspaces {
    Table,
    Id,
}
