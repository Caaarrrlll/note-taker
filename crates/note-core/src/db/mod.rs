//! Database connection setup.
//!
//! The backend (SQLite, PostgreSQL or MySQL/MariaDB) is chosen by the scheme of
//! the connection URL. The connection is managed as Tauri state (see `lib.rs`)
//! and passed explicitly into services, which keeps them testable.

use sea_orm::{ConnectOptions, Database, DatabaseConnection, DbErr};
use sea_orm_migration::MigratorTrait;
use std::path::Path;

use crate::db::migrations::Migrator;

pub mod migrations;

/// File name of the default local SQLite database inside the app data dir.
pub const DEFAULT_SQLITE_FILE: &str = "notes.db";

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: Option<u32>,
}

impl DbConfig {
    /// Build config from environment variables, falling back to a local SQLite
    /// database in `default_dir` when `DATABASE_URL` is not set.
    pub fn from_env(default_dir: &Path) -> Result<Self, String> {
        let url = match std::env::var("DATABASE_URL") {
            Ok(url) if !url.trim().is_empty() => url,
            _ => sqlite_url_for(&default_dir.join(DEFAULT_SQLITE_FILE)),
        };

        let max_connections = match std::env::var("DATABASE_MAX_CONNECTIONS") {
            Ok(v) if !v.trim().is_empty() => Some(
                v.trim()
                    .parse()
                    .map_err(|_| format!("DATABASE_MAX_CONNECTIONS must be a number, got {v:?}"))?,
            ),
            _ => None,
        };

        Ok(Self { url, max_connections })
    }
}

/// `sqlite://` URL for a file path, creating the file if it does not exist.
pub fn sqlite_url_for(path: &Path) -> String {
    // Forward slashes keep Windows paths (C:\...) valid inside a URL.
    let path = path.to_string_lossy().replace('\\', "/");
    format!("sqlite://{path}?mode=rwc")
}

/// Human-readable backend name for logs, without credentials.
pub fn backend_name(url: &str) -> &'static str {
    match url.split(':').next().unwrap_or_default() {
        "sqlite" => "SQLite",
        "postgres" | "postgresql" => "PostgreSQL",
        "mysql" | "mariadb" => "MySQL/MariaDB",
        _ => "unknown",
    }
}

/// URL in the form the driver accepts. MariaDB speaks the MySQL protocol and
/// uses the MySQL driver, which only recognises `mysql://`; accept the more
/// natural `mariadb://` too. (`postgresql://` is already accepted as-is.)
pub fn driver_url(url: &str) -> String {
    match url.strip_prefix("mariadb://") {
        Some(rest) => format!("mysql://{rest}"),
        None => url.to_string(),
    }
}

/// Connect and bring the schema up to date.
pub async fn connect(config: &DbConfig) -> Result<DatabaseConnection, DbErr> {
    let mut opts = ConnectOptions::new(driver_url(&config.url));
    opts.sqlx_logging(false);

    let is_sqlite_memory = config.url.starts_with("sqlite") && config.url.contains(":memory:");
    if is_sqlite_memory {
        // Each in-memory SQLite connection is a separate database: use exactly one.
        opts.max_connections(1).min_connections(1);
    } else if let Some(n) = config.max_connections {
        opts.max_connections(n);
    }

    let conn = Database::connect(opts).await?;
    Migrator::up(&conn, None).await?;
    Ok(conn)
}
