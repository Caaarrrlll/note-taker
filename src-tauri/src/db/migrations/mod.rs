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
