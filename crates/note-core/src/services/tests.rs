//! Service-level integration tests.
//!
//! By default they run against a fresh in-memory SQLite database per test.
//! Set `TEST_DATABASE_URL` to run them against an existing PostgreSQL or
//! MySQL/MariaDB server (use a throwaway database: tests create rows and
//! never clean up). Tests only use random UUIDs, so they can share one
//! server database and run in parallel.

use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, ModelTrait, Set};
use serde_json::json;
use uuid::Uuid;

use super::auth_service::*;
use super::block_service::*;
use super::document_service::*;
use super::workspace_service::*;
use crate::db::{self, DbConfig};
use crate::entities::{blocks, documents, users, workspaces};
use crate::error::AppError;

/// Serialises migrations when several tests hit the same server database at once.
static MIGRATE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn is_shared_server_db() -> bool {
    std::env::var("TEST_DATABASE_URL").is_ok()
}

async fn test_db() -> DatabaseConnection {
    let url = std::env::var("TEST_DATABASE_URL").unwrap_or_else(|_| "sqlite::memory:".into());
    let _guard = MIGRATE_LOCK.lock().await;
    db::connect(&DbConfig { url, max_connections: Some(2) })
        .await
        .expect("connect to test database")
}

async fn seed_user(db: &DatabaseConnection) -> users::Model {
    let id = Uuid::new_v4();
    users::ActiveModel {
        id: Set(id),
        email: Set(format!("{id}@test.local")),
        display_name: Set("Test User".into()),
        password_hash: Set(None),
        is_local: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("insert user")
}

/// A user with a workspace: (user_id, workspace).
async fn seed_workspace(db: &DatabaseConnection) -> (Uuid, workspaces::Model) {
    let user = seed_user(db).await;
    let ws = create_workspace(db, user.id, "Test Workspace").await.expect("create workspace");
    (user.id, ws)
}

fn block(id: Option<&str>, kind: &str, text: &str) -> BlockInput {
    BlockInput {
        id: id.map(String::from),
        block_type: kind.into(),
        content: json!({ "type": kind, "content": [{ "type": "text", "text": text }] }),
    }
}

// ---------------------------------------------------------------- auth / users

#[tokio::test]
async fn local_user_and_default_workspace_are_created_once() {
    let db = test_db().await;
    let a = ensure_local_user(&db).await.unwrap();
    let b = ensure_local_user(&db).await.unwrap();
    assert_eq!(a.id, b.id);
    assert!(a.is_local && a.password_hash.is_none());

    let ws1 = ensure_default_workspace(&db, a.id).await.unwrap();
    let ws2 = ensure_default_workspace(&db, a.id).await.unwrap();
    assert_eq!(ws1.id, ws2.id);
    assert_eq!(ws1.name, DEFAULT_WORKSPACE_NAME);
    if !is_shared_server_db() {
        assert_eq!(list_user_workspaces(&db, a.id).await.unwrap().len(), 1);
    }
}

#[test]
fn password_hash_round_trip() {
    let hash = hash_password("correct horse battery").unwrap();
    assert!(hash.starts_with("$argon2"));
    assert!(verify_password("correct horse battery", &hash));
    assert!(!verify_password("wrong", &hash));
    assert!(!verify_password("anything", "not-a-hash"));
}

#[tokio::test]
async fn register_and_login() {
    let db = test_db().await;
    let email = format!("{}@Example.com", Uuid::new_v4());

    let user = register(&db, &format!("  {email} "), "Alice", "s3cret-pass").await.unwrap();
    assert_eq!(user.email, email.to_lowercase(), "email is trimmed and lowercased");
    assert!(!user.is_local);

    let logged_in = login(&db, &email.to_uppercase(), "s3cret-pass").await.unwrap();
    assert_eq!(logged_in.id, user.id);

    assert!(matches!(login(&db, &email, "wrong-pass").await, Err(AppError::InvalidCredentials)));
    assert!(matches!(login(&db, "nobody@example.com", "s3cret-pass").await, Err(AppError::InvalidCredentials)));
    assert!(matches!(register(&db, &email, "Again", "s3cret-pass").await, Err(AppError::Invalid(_))));
    assert!(matches!(register(&db, "bad-email", "Bob", "s3cret-pass").await, Err(AppError::Invalid(_))));
    assert!(matches!(register(&db, "b@example.com", "Bob", "short").await, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn concurrent_first_run_creates_one_local_user() {
    let db = test_db().await;
    let (a, b, c, d) = tokio::join!(
        ensure_local_user(&db),
        ensure_local_user(&db),
        ensure_local_user(&db),
        ensure_local_user(&db),
    );
    let ids: Vec<Uuid> = [a, b, c, d].into_iter().map(|r| r.expect("ensure_local_user").id).collect();
    assert!(ids.iter().all(|id| *id == ids[0]), "all callers get the same local user");
}

#[tokio::test]
async fn local_user_cannot_log_in_with_a_password() {
    let db = test_db().await;
    ensure_local_user(&db).await.unwrap();
    assert!(matches!(login(&db, LOCAL_USER_EMAIL, "").await, Err(AppError::InvalidCredentials)));
}

// ------------------------------------------------------------------ workspaces

#[tokio::test]
async fn workspaces_are_scoped_to_their_owner() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let (other, _) = seed_workspace(&db).await;

    let listed = list_user_workspaces(&db, user).await.unwrap();
    assert_eq!(listed.iter().map(|w| w.id).collect::<Vec<_>>(), vec![ws.id]);
    assert!(get_owned_workspace(&db, user, ws.id).await.is_ok());
    assert!(matches!(get_owned_workspace(&db, other, ws.id).await, Err(AppError::NotFound(_))));
    assert!(matches!(create_workspace(&db, user, "   ").await, Err(AppError::Invalid(_))));
}

// ------------------------------------------------------------------- documents

#[tokio::test]
async fn documents_crud_and_nesting() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;

    let root = create_document(&db, user, ws.id, "Root", None).await.unwrap();
    let child = create_document(&db, user, ws.id, "Child", Some(root.id)).await.unwrap();
    assert_eq!(child.parent_id, Some(root.id));

    let blank = create_document(&db, user, ws.id, "   ", None).await.unwrap();
    assert_eq!(blank.title, "", "blank titles are trimmed to empty");

    assert_eq!(get_owned_document(&db, user, root.id).await.unwrap().title, "Root");
    assert!(matches!(get_owned_document(&db, user, Uuid::new_v4()).await, Err(AppError::NotFound(_))));
    assert_eq!(list_documents(&db, user, ws.id).await.unwrap().len(), 3);

    let renamed = update_document_title(&db, user, root.id, "Renamed").await.unwrap();
    assert_eq!(renamed.title, "Renamed");
    assert!(matches!(
        update_document_title(&db, user, root.id, &"x".repeat(256)).await,
        Err(AppError::Invalid(_))
    ));
}

#[tokio::test]
async fn other_users_documents_are_invisible() {
    let db = test_db().await;
    let (owner, ws) = seed_workspace(&db).await;
    let (intruder, intruder_ws) = seed_workspace(&db).await;
    let doc = create_document(&db, owner, ws.id, "Private", None).await.unwrap();

    assert!(matches!(get_owned_document(&db, intruder, doc.id).await, Err(AppError::NotFound(_))));
    assert!(matches!(list_documents(&db, intruder, ws.id).await, Err(AppError::NotFound(_))));
    assert!(update_document_title(&db, intruder, doc.id, "Hacked").await.is_err());
    assert!(archive_document(&db, intruder, doc.id).await.is_err());
    assert!(get_document_blocks(&db, intruder, doc.id).await.is_err());
    assert!(save_document_blocks(&db, intruder, doc.id, vec![]).await.is_err());
    // Can't nest your page under someone else's.
    assert!(create_document(&db, intruder, intruder_ws.id, "Sneaky", Some(doc.id)).await.is_err());
}

#[tokio::test]
async fn archiving_a_page_archives_its_descendants() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let a = create_document(&db, user, ws.id, "A", None).await.unwrap();
    let b = create_document(&db, user, ws.id, "B", Some(a.id)).await.unwrap();
    let c = create_document(&db, user, ws.id, "C", Some(b.id)).await.unwrap();
    let other = create_document(&db, user, ws.id, "Other", None).await.unwrap();

    let mut archived = archive_document(&db, user, a.id).await.unwrap();
    archived.sort();
    let mut expected = vec![a.id, b.id, c.id];
    expected.sort();
    assert_eq!(archived, expected);

    let remaining = list_documents(&db, user, ws.id).await.unwrap();
    assert_eq!(remaining.iter().map(|d| d.id).collect::<Vec<_>>(), vec![other.id]);
}

#[tokio::test]
async fn trash_lists_restores_and_permanently_deletes() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let a = create_document(&db, user, ws.id, "A", None).await.unwrap();
    let b = create_document(&db, user, ws.id, "B", Some(a.id)).await.unwrap();
    let c = create_document(&db, user, ws.id, "C", Some(b.id)).await.unwrap();
    save_document_blocks(&db, user, c.id, vec![block(None, "paragraph", "deep content")]).await.unwrap();
    let c_block = get_document_blocks(&db, user, c.id).await.unwrap()[0].id;

    archive_document(&db, user, a.id).await.unwrap();
    let trash: Vec<Uuid> = list_archived_documents(&db, user, ws.id).await.unwrap().iter().map(|d| d.id).collect();
    assert_eq!(trash.len(), 3);
    assert!(trash.contains(&a.id) && trash.contains(&b.id) && trash.contains(&c.id));

    // Restoring B while its parent A is still in the trash: B (and C) come back at the top level.
    let restored = restore_document(&db, user, b.id).await.unwrap();
    let mut restored_ids: Vec<Uuid> = restored.iter().map(|d| d.id).collect();
    restored_ids.sort();
    let mut expected = vec![b.id, c.id];
    expected.sort();
    assert_eq!(restored_ids, expected);
    let b_now = get_owned_document(&db, user, b.id).await.unwrap();
    assert_eq!(b_now.parent_id, None, "restored to top level because its parent is in the trash");
    assert!(!b_now.is_archived);
    assert_eq!(get_owned_document(&db, user, c.id).await.unwrap().parent_id, Some(b.id), "C stays under B");
    assert_eq!(get_document_blocks(&db, user, c.id).await.unwrap().len(), 1, "content survives the trash");

    // Only trashed pages can be deleted permanently.
    assert!(matches!(delete_document_permanently(&db, user, b.id).await, Err(AppError::Invalid(_))));

    // Re-trash B (with C), then delete it for good: pages and their blocks are gone.
    archive_document(&db, user, b.id).await.unwrap();
    let mut deleted = delete_document_permanently(&db, user, b.id).await.unwrap();
    deleted.sort();
    assert_eq!(deleted, expected);
    assert!(documents::Entity::find_by_id(b.id).one(&db).await.unwrap().is_none());
    assert!(documents::Entity::find_by_id(c.id).one(&db).await.unwrap().is_none());
    assert!(blocks::Entity::find_by_id(c_block).one(&db).await.unwrap().is_none());

    // A is still in the trash and restores normally.
    assert_eq!(list_archived_documents(&db, user, ws.id).await.unwrap().len(), 1);
    restore_document(&db, user, a.id).await.unwrap();
    assert!(list_archived_documents(&db, user, ws.id).await.unwrap().is_empty());
}

#[tokio::test]
async fn trash_is_scoped_to_the_owner() {
    let db = test_db().await;
    let (owner, ws) = seed_workspace(&db).await;
    let (intruder, _) = seed_workspace(&db).await;
    let doc = create_document(&db, owner, ws.id, "Private", None).await.unwrap();
    archive_document(&db, owner, doc.id).await.unwrap();

    assert!(list_archived_documents(&db, intruder, ws.id).await.is_err());
    assert!(restore_document(&db, intruder, doc.id).await.is_err());
    assert!(delete_document_permanently(&db, intruder, doc.id).await.is_err());
    assert!(get_owned_document(&db, owner, doc.id).await.unwrap().is_archived);
}

#[tokio::test]
async fn moving_pages_and_cycle_protection() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let a = create_document(&db, user, ws.id, "A", None).await.unwrap();
    let b = create_document(&db, user, ws.id, "B", Some(a.id)).await.unwrap();
    let c = create_document(&db, user, ws.id, "C", Some(b.id)).await.unwrap();

    // Into itself or into its own descendant: rejected.
    assert!(matches!(move_document(&db, user, a.id, Some(a.id)).await, Err(AppError::Invalid(_))));
    assert!(matches!(move_document(&db, user, a.id, Some(c.id)).await, Err(AppError::Invalid(_))));

    // To top level, then under a sibling: allowed.
    assert_eq!(move_document(&db, user, c.id, None).await.unwrap().parent_id, None);
    assert_eq!(move_document(&db, user, a.id, Some(c.id)).await.unwrap().parent_id, Some(c.id));

    // Across workspaces: rejected.
    let ws2 = create_workspace(&db, user, "Second").await.unwrap();
    let elsewhere = create_document(&db, user, ws2.id, "Elsewhere", None).await.unwrap();
    assert!(matches!(move_document(&db, user, b.id, Some(elsewhere.id)).await, Err(AppError::Invalid(_))));
}

#[tokio::test]
async fn timestamps_are_set_on_insert_and_bumped_on_update() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;

    let before = chrono::Utc::now() - chrono::Duration::seconds(5);
    let doc = create_document(&db, user, ws.id, "Old title", None).await.unwrap();
    assert!(doc.created_at >= before, "created_at should be set to now");
    assert!(doc.updated_at >= doc.created_at);

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    let updated = update_document_title(&db, user, doc.id, "New title").await.unwrap();
    assert_eq!(updated.created_at, doc.created_at, "created_at must not change");
    assert!(updated.updated_at > doc.updated_at, "updated_at must move forward");
}

// ---------------------------------------------------------------------- blocks

#[tokio::test]
async fn saving_blocks_inserts_updates_reorders_and_deletes() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let doc = create_document(&db, user, ws.id, "Page", None).await.unwrap();
    let (id1, id2, id3) = (Uuid::new_v4().to_string(), Uuid::new_v4().to_string(), Uuid::new_v4().to_string());

    save_document_blocks(&db, user, doc.id, vec![
        block(Some(&id1), "heading", "Title ✓ ünïcödé"),
        block(Some(&id2), "paragraph", "First"),
        block(Some(&id3), "paragraph", "Second"),
    ])
    .await
    .unwrap();

    let saved = get_document_blocks(&db, user, doc.id).await.unwrap();
    assert_eq!(saved.iter().map(|b| b.id.to_string()).collect::<Vec<_>>(), vec![id1.clone(), id2.clone(), id3.clone()]);
    assert_eq!(saved[0].content, block(None, "heading", "Title ✓ ünïcödé").content);
    let created_at_of_1 = saved[0].created_at;

    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // Reorder, edit one, delete one, add one.
    let id4 = Uuid::new_v4().to_string();
    save_document_blocks(&db, user, doc.id, vec![
        block(Some(&id3), "paragraph", "Second (edited)"),
        block(Some(&id1), "heading", "Title ✓ ünïcödé"),
        block(Some(&id4), "bulletListItem", "New"),
    ])
    .await
    .unwrap();

    let saved = get_document_blocks(&db, user, doc.id).await.unwrap();
    assert_eq!(saved.iter().map(|b| b.id.to_string()).collect::<Vec<_>>(), vec![id3, id1, id4]);
    assert_eq!(saved[0].content, block(None, "paragraph", "Second (edited)").content);
    assert_eq!(saved[1].created_at, created_at_of_1, "existing blocks keep created_at");
    assert_eq!(saved[2].block_type, "bulletListItem");

    // Saving an empty page clears it, and the page's updated_at moves forward.
    save_document_blocks(&db, user, doc.id, vec![]).await.unwrap();
    assert!(get_document_blocks(&db, user, doc.id).await.unwrap().is_empty());
    let doc_after = get_owned_document(&db, user, doc.id).await.unwrap();
    assert!(doc_after.updated_at > doc.updated_at);
}

#[tokio::test]
async fn saving_blocks_handles_bad_duplicate_and_foreign_ids() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let page1 = create_document(&db, user, ws.id, "One", None).await.unwrap();
    let page2 = create_document(&db, user, ws.id, "Two", None).await.unwrap();

    let shared = Uuid::new_v4().to_string();
    save_document_blocks(&db, user, page1.id, vec![block(Some(&shared), "paragraph", "On page 1")])
        .await
        .unwrap();

    // Same ID as a block on page 1, a duplicate within the save, a non-UUID ID and no ID.
    let dup = Uuid::new_v4().to_string();
    save_document_blocks(&db, user, page2.id, vec![
        block(Some(&shared), "paragraph", "Copied"),
        block(Some(&dup), "paragraph", "A"),
        block(Some(&dup), "paragraph", "B"),
        block(Some("not-a-uuid"), "paragraph", "C"),
        block(None, "paragraph", "D"),
    ])
    .await
    .unwrap();

    let p2 = get_document_blocks(&db, user, page2.id).await.unwrap();
    assert_eq!(p2.len(), 5, "every block is stored");
    assert!(p2.iter().all(|b| b.id.to_string() != shared), "page 1's block ID is not stolen");
    assert_eq!(p2[1].id.to_string(), dup, "first occurrence keeps its ID");
    let p1 = get_document_blocks(&db, user, page1.id).await.unwrap();
    assert_eq!(p1.len(), 1, "page 1 is untouched");
    assert_eq!(p1[0].id.to_string(), shared);

    assert!(matches!(
        save_document_blocks(&db, user, page2.id, vec![block(None, "", "x")]).await,
        Err(AppError::Invalid(_))
    ));
}

#[tokio::test]
async fn deleting_a_workspace_cascades_to_documents_and_blocks() {
    let db = test_db().await;
    let (user, ws) = seed_workspace(&db).await;
    let root = create_document(&db, user, ws.id, "Root", None).await.unwrap();
    let child = create_document(&db, user, ws.id, "Child", Some(root.id)).await.unwrap();
    save_document_blocks(&db, user, child.id, vec![block(None, "paragraph", "x")]).await.unwrap();
    let block_id = get_document_blocks(&db, user, child.id).await.unwrap()[0].id;

    ws.delete(&db).await.unwrap();

    // Proves foreign keys are enforced on every backend (SQLite needs them switched on).
    assert!(documents::Entity::find_by_id(root.id).one(&db).await.unwrap().is_none());
    assert!(documents::Entity::find_by_id(child.id).one(&db).await.unwrap().is_none());
    assert!(blocks::Entity::find_by_id(block_id).one(&db).await.unwrap().is_none());
}

#[test]
fn mariadb_urls_use_the_mysql_driver() {
    assert_eq!(db::driver_url("mariadb://u:p@host:3306/notes"), "mysql://u:p@host:3306/notes");
    assert_eq!(db::driver_url("mysql://u:p@host:3306/notes"), "mysql://u:p@host:3306/notes");
    assert_eq!(db::driver_url("postgresql://u:p@host/notes"), "postgresql://u:p@host/notes");
    assert_eq!(db::backend_name("mariadb://u:p@host/notes"), "MySQL/MariaDB");
}

#[test]
fn sqlite_url_from_windows_path() {
    let url = db::sqlite_url_for(std::path::Path::new(r"C:\Users\me\AppData\Roaming\app\notes.db"));
    assert_eq!(url, "sqlite://C:/Users/me/AppData/Roaming/app/notes.db?mode=rwc");
}

#[test]
fn errors_serialize_with_kind_and_message() {
    let v = serde_json::to_value(AppError::NotFound("Document")).unwrap();
    assert_eq!(v, json!({ "kind": "notFound", "message": "Document not found" }));
}
