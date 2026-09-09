use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Workspaces::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Workspaces::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Workspaces::Name).string().not_null())
                    .col(ColumnDef::new(Workspaces::OwnerId).uuid().not_null())
                    .col(ColumnDef::new(Workspaces::CreatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .col(ColumnDef::new(Workspaces::UpdatedAt).timestamp_with_time_zone().not_null().default(Expr::current_timestamp()))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_workspace_owner")
                            .from(Workspaces::Table, Workspaces::OwnerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Workspaces::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Workspaces {
    Table,
    Id,
    Name,
    OwnerId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}
