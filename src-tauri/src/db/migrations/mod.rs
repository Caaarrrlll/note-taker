use sea_orm_migration::prelude::*;

mod m20260909_000001_create_users;
mod m20260909_000002_create_workspaces;
mod m20260909_000003_create_documents;
mod m20260909_000004_create_blocks;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260909_000001_create_users::Migration),
            Box::new(m20260909_000002_create_workspaces::Migration),
            Box::new(m20260909_000003_create_documents::Migration),
            Box::new(m20260909_000004_create_blocks::Migration),
        ]
    }
}

/// A NOT NULL UTC timestamp column that maps to `DateTimeUtc` on every backend.
///
/// MySQL/MariaDB `TIMESTAMP` overflows in 2038 and drops sub-second precision,
/// so use `DATETIME(6)` there. Values are always set in code (see
/// `entities::touch_timestamps!`), so no DB default is needed.
pub(crate) fn utc_timestamp<T: IntoIden>(manager: &SchemaManager, col: T) -> ColumnDef {
    let mut def = ColumnDef::new(col);
    match manager.get_database_backend() {
        sea_orm::DbBackend::MySql => def.custom(Alias::new("DATETIME(6)")),
        _ => def.timestamp_with_time_zone(),
    };
    def.not_null();
    def
}
