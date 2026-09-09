use sea_orm::{Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use std::sync::OnceLock;

use crate::db::migrations::Migrator;

pub mod migrations;

static DB: OnceLock<DatabaseConnection> = OnceLock::new();

pub async fn init(database_url: &str) -> Result<(), DbErr> {
    let conn = Database::connect(database_url).await?;
    Migrator::up(&conn, None).await?;
    DB.set(conn)
        .map_err(|_| DbErr::Custom("Database already initialized".into()))?;
    Ok(())
}

pub fn get_db() -> &'static DatabaseConnection {
    DB.get().expect("Database not initialized — call db::init() first")
}
