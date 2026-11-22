use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Users::Email).string_len(255).not_null().unique_key())
                    .col(ColumnDef::new(Users::DisplayName).string_len(255).not_null())
                    // NULL for the passwordless local user (see services::auth_service).
                    .col(ColumnDef::new(Users::PasswordHash).string_len(255).null())
                    .col(ColumnDef::new(Users::IsLocal).boolean().not_null().default(false))
                    .col(super::utc_timestamp(manager, Users::CreatedAt))
                    .col(super::utc_timestamp(manager, Users::UpdatedAt))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
    Email,
    DisplayName,
    PasswordHash,
    IsLocal,
    CreatedAt,
    UpdatedAt,
}
