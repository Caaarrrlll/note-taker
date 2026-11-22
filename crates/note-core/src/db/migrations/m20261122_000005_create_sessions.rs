use sea_orm_migration::prelude::*;

/// Login sessions for the web server. Only a SHA-256 hash of each token is
/// stored, so a leaked database can't be used to take over sessions.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::TokenHash).string_len(64).not_null().primary_key())
                    .col(ColumnDef::new(Sessions::UserId).uuid().not_null())
                    .col(super::utc_timestamp(manager, Sessions::CreatedAt))
                    .col(super::utc_timestamp(manager, Sessions::ExpiresAt))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_session_user")
                            .from(Sessions::Table, Sessions::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum Sessions {
    Table,
    TokenHash,
    UserId,
    CreatedAt,
    ExpiresAt,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}
