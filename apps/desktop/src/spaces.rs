//! Spaces: separate sets of notes (e.g. Work, Personal, School), each stored in
//! its own local SQLite file (ROADMAP Phase 6).
//!
//! The space list lives in `spaces.json` in the app config dir, because it has
//! to exist before any database is opened. Each space's database keeps its own
//! local user and workspace, created on first open. Connections are opened
//! lazily, one pool per space.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, EntityTrait, PaginatorTrait, Statement};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::Mutex as AsyncMutex;
use uuid::Uuid;

use note_core::db::{self, DbConfig};
use note_core::entities::{blocks, documents, users, workspaces};
use note_core::error::{AppError, AppResult};
use note_core::services::{auth_service, workspace_service};

/// File name of the first space, so installs from before spaces keep their notes.
const LEGACY_DB_FILE: &str = db::DEFAULT_SQLITE_FILE;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Space {
    pub id: String,
    pub name: String,
    /// `#rrggbb`
    pub color: String,
    /// The space's SQLite file.
    pub path: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
struct SpacesFile {
    spaces: Vec<Space>,
}

/// An opened space: its database and the local user inside it.
#[derive(Clone)]
pub struct OpenSpace {
    pub db: DatabaseConnection,
    pub user_id: Uuid,
}

pub struct Spaces {
    config_path: PathBuf,
    data_dir: PathBuf,
    /// `DATABASE_URL` from the environment: a dev/CI override for the first space.
    first_space_override: Option<DbConfig>,
    list: Mutex<Vec<Space>>,
    open: AsyncMutex<HashMap<String, OpenSpace>>,
}

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 100 {
        return Err(AppError::Invalid("Space name must be 1–100 characters".into()));
    }
    Ok(name.to_string())
}

fn validate_color(color: &str) -> AppResult<String> {
    let ok = color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(AppError::Invalid("Colour must look like #3b82f6".into()));
    }
    Ok(color.to_lowercase())
}

/// Same file, ignoring case and separator differences (Windows paths).
fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('\\', "/").to_lowercase();
    norm(a) == norm(b)
}

impl Spaces {
    /// Load `spaces.json`, creating it on first run with one "Personal" space
    /// on the existing `notes.db` (so earlier installs keep their notes).
    pub fn load(config_path: PathBuf, data_dir: PathBuf, first_space_override: Option<DbConfig>) -> Result<Self, String> {
        let spaces = match std::fs::read_to_string(&config_path) {
            Ok(text) => {
                serde_json::from_str::<SpacesFile>(&text)
                    .map_err(|e| format!("{} is not valid: {e}", config_path.display()))?
                    .spaces
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(format!("Cannot read {}: {e}", config_path.display())),
        };

        let this = Self {
            config_path,
            data_dir,
            first_space_override,
            list: Mutex::new(spaces),
            open: AsyncMutex::new(HashMap::new()),
        };

        if this.list().is_empty() {
            let personal = Space {
                id: Uuid::new_v4().to_string(),
                name: "Personal".into(),
                color: "#3b82f6".into(),
                path: this.data_dir.join(LEGACY_DB_FILE),
            };
            this.list.lock().unwrap_or_else(|e| e.into_inner()).push(personal);
            this.save().map_err(|e| e.to_string())?;
        }
        Ok(this)
    }

    pub fn list(&self) -> Vec<Space> {
        self.list.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn get(&self, id: &str) -> AppResult<Space> {
        self.list()
            .into_iter()
            .find(|s| s.id == id)
            .ok_or(AppError::NotFound("Space"))
    }

    fn save(&self) -> AppResult<()> {
        let file = SpacesFile { spaces: self.list() };
        let text = serde_json::to_string_pretty(&file).map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(dir) = self.config_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| AppError::Internal(format!("Cannot create config dir: {e}")))?;
        }
        // Write a temp file, then rename, so a crash never leaves half a file.
        let tmp = self.config_path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| AppError::Internal(format!("Cannot write spaces.json: {e}")))?;
        std::fs::rename(&tmp, &self.config_path)
            .map_err(|e| AppError::Internal(format!("Cannot write spaces.json: {e}")))?;
        Ok(())
    }

    fn db_config(&self, space: &Space) -> DbConfig {
        let is_first = self.list().first().is_some_and(|s| s.id == space.id);
        match (&self.first_space_override, is_first) {
            (Some(config), true) => config.clone(),
            _ => DbConfig { url: db::sqlite_url_for(&space.path), max_connections: None },
        }
    }

    async fn connect(config: &DbConfig, path: Option<&Path>) -> AppResult<OpenSpace> {
        if let Some(dir) = path.and_then(Path::parent) {
            std::fs::create_dir_all(dir)
                .map_err(|e| AppError::Invalid(format!("Cannot create folder {}: {e}", dir.display())))?;
        }
        let db = db::connect(config).await?;
        let user = auth_service::ensure_local_user(&db).await?;
        workspace_service::ensure_default_workspace(&db, user.id).await?;
        Ok(OpenSpace { db, user_id: user.id })
    }

    /// The space's database, opening it (and creating it on first use) if needed.
    pub async fn open(&self, id: &str) -> AppResult<OpenSpace> {
        let mut open = self.open.lock().await;
        if let Some(space) = open.get(id) {
            return Ok(space.clone());
        }
        let space = self.get(id)?;
        let opened = Self::connect(&self.db_config(&space), Some(&space.path)).await?;
        open.insert(space.id, opened.clone());
        Ok(opened)
    }

    async fn close(&self, id: &str) {
        if let Some(space) = self.open.lock().await.remove(id) {
            let _ = space.db.close().await;
        }
    }

    fn ensure_path_unused(&self, path: &Path, except_id: Option<&str>) -> AppResult<()> {
        let taken = self
            .list()
            .iter()
            .any(|s| Some(s.id.as_str()) != except_id && same_path(&s.path, path));
        if taken {
            return Err(AppError::Invalid("Another space already uses that file".into()));
        }
        Ok(())
    }

    /// Create a space. `path` is an existing SQLite file to open, or a new file to
    /// create; without it the file goes in the app data dir.
    pub async fn create(&self, name: &str, color: &str, path: Option<PathBuf>) -> AppResult<Space> {
        let id = Uuid::new_v4().to_string();
        let path = path.unwrap_or_else(|| self.data_dir.join(format!("space-{id}.db")));
        self.ensure_path_unused(&path, None)?;
        let space = Space { id, name: validate_name(name)?, color: validate_color(color)?, path };

        self.list.lock().unwrap_or_else(|e| e.into_inner()).push(space.clone());
        // Open right away, so a bad location or a file that isn't a notes
        // database is reported now, not on first use.
        if let Err(e) = self.open(&space.id).await {
            self.list.lock().unwrap_or_else(|e| e.into_inner()).retain(|s| s.id != space.id);
            return Err(e);
        }
        self.save()?;
        Ok(space)
    }

    pub fn update(&self, id: &str, name: &str, color: &str) -> AppResult<Space> {
        let (name, color) = (validate_name(name)?, validate_color(color)?);
        let updated = {
            let mut list = self.list.lock().unwrap_or_else(|e| e.into_inner());
            let space = list.iter_mut().find(|s| s.id == id).ok_or(AppError::NotFound("Space"))?;
            space.name = name;
            space.color = color;
            space.clone()
        };
        self.save()?;
        Ok(updated)
    }

    /// Remove a space from the list. Its database file is kept on disk.
    pub async fn remove(&self, id: &str) -> AppResult<()> {
        self.get(id)?;
        if self.list().len() == 1 {
            return Err(AppError::Invalid("You can't remove the only space".into()));
        }
        self.close(id).await;
        self.list.lock().unwrap_or_else(|e| e.into_inner()).retain(|s| s.id != id);
        self.save()
    }

    /// Move a space's database to `new_path`: copy it with `VACUUM INTO`, check
    /// that every table has the same row count, then switch over. The old file
    /// is kept until the user deletes it.
    pub async fn move_file(&self, id: &str, new_path: PathBuf) -> AppResult<Space> {
        let space = self.get(id)?;
        if self.first_space_override.is_some() && self.list().first().is_some_and(|s| s.id == id) {
            return Err(AppError::Invalid("This space uses DATABASE_URL and can't be moved".into()));
        }
        if new_path.exists() {
            return Err(AppError::Invalid("A file already exists there; choose a new file name".into()));
        }
        self.ensure_path_unused(&new_path, Some(id))?;
        if let Some(dir) = new_path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| AppError::Invalid(format!("Cannot create folder {}: {e}", dir.display())))?;
        }

        let old = self.open(id).await?;
        // SQLite-only by design: spaces in this phase are always SQLite files.
        old.db
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "VACUUM INTO ?",
                [new_path.to_string_lossy().to_string().into()],
            ))
            .await?;

        let config = DbConfig { url: db::sqlite_url_for(&new_path), max_connections: None };
        let new = Self::connect(&config, None).await?;
        if row_counts(&old.db).await? != row_counts(&new.db).await? {
            let _ = new.db.close().await;
            let _ = std::fs::remove_file(&new_path);
            return Err(AppError::Internal("The copy didn't match the original; nothing was changed".into()));
        }

        let moved = {
            let mut list = self.list.lock().unwrap_or_else(|e| e.into_inner());
            let s = list.iter_mut().find(|s| s.id == space.id).ok_or(AppError::NotFound("Space"))?;
            s.path = new_path;
            s.clone()
        };
        self.save()?;
        let mut open = self.open.lock().await;
        if let Some(previous) = open.insert(space.id, new) {
            let _ = previous.db.close().await;
        }
        Ok(moved)
    }
}

async fn row_counts(db: &DatabaseConnection) -> AppResult<[u64; 4]> {
    Ok([
        users::Entity::find().count(db).await?,
        workspaces::Entity::find().count(db).await?,
        documents::Entity::find().count(db).await?,
        blocks::Entity::find().count(db).await?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use note_core::services::document_service;

    fn temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("note-taker-spaces-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn load(dir: &Path) -> Spaces {
        Spaces::load(dir.join("config").join("spaces.json"), dir.join("data"), None).unwrap()
    }

    async fn workspace_id(space: &OpenSpace) -> Uuid {
        workspace_service::list_user_workspaces(&space.db, space.user_id).await.unwrap()[0].id
    }

    #[test]
    fn first_run_creates_personal_on_the_legacy_file_and_persists() {
        let dir = temp_dir();
        let spaces = load(&dir);
        let list = spaces.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Personal");
        assert_eq!(list[0].path, dir.join("data").join(LEGACY_DB_FILE));
        assert_eq!(load(&dir).list(), list, "reloading gives the same spaces");
    }

    #[test]
    fn spaces_are_isolated_and_survive_a_restart() {
        tauri::async_runtime::block_on(async {
            let dir = temp_dir();
            let spaces = load(&dir);
            let personal = spaces.list()[0].id.clone();
            let work = spaces.create("Work", "#EF4444", None).await.unwrap();
            assert_eq!(work.color, "#ef4444");

            let p = spaces.open(&personal).await.unwrap();
            let ws = workspace_id(&p).await;
            document_service::create_document(&p.db, p.user_id, ws, "Private note", None).await.unwrap();

            let w = spaces.open(&work.id).await.unwrap();
            let work_ws = workspace_id(&w).await;
            assert!(document_service::list_documents(&w.db, w.user_id, work_ws).await.unwrap().is_empty());

            let reloaded = load(&dir);
            assert_eq!(reloaded.list().len(), 2);
            let p2 = reloaded.open(&personal).await.unwrap();
            let docs = document_service::list_documents(&p2.db, p2.user_id, workspace_id(&p2).await).await.unwrap();
            assert_eq!(docs.len(), 1);
        });
    }

    #[test]
    fn creating_validates_input_and_rejects_a_file_already_in_use() {
        tauri::async_runtime::block_on(async {
            let dir = temp_dir();
            let spaces = load(&dir);
            let used = spaces.list()[0].path.clone();
            assert!(matches!(spaces.create("Dup", "#000000", Some(used)).await, Err(AppError::Invalid(_))));
            assert!(matches!(spaces.create("  ", "#000000", None).await, Err(AppError::Invalid(_))));
            assert!(matches!(spaces.create("Bad colour", "blue", None).await, Err(AppError::Invalid(_))));
            assert_eq!(spaces.list().len(), 1, "failed creates leave the list unchanged");
        });
    }

    #[test]
    fn opening_an_existing_file_creates_a_space_with_its_notes() {
        tauri::async_runtime::block_on(async {
            let dir = temp_dir();
            let spaces = load(&dir);
            let school = spaces.create("School", "#22c55e", Some(dir.join("elsewhere").join("school.db"))).await.unwrap();
            let s = spaces.open(&school.id).await.unwrap();
            document_service::create_document(&s.db, s.user_id, workspace_id(&s).await, "Lecture 1", None)
                .await
                .unwrap();
            spaces.remove(&school.id).await.unwrap();
            assert!(school.path.exists(), "removing a space keeps its file");

            let again = spaces.create("School again", "#22c55e", Some(school.path.clone())).await.unwrap();
            let a = spaces.open(&again.id).await.unwrap();
            let docs = document_service::list_documents(&a.db, a.user_id, workspace_id(&a).await).await.unwrap();
            assert_eq!(docs[0].title, "Lecture 1");
        });
    }

    #[test]
    fn moving_a_file_copies_verifies_and_keeps_the_old_file() {
        tauri::async_runtime::block_on(async {
            let dir = temp_dir();
            let spaces = load(&dir);
            let id = spaces.list()[0].id.clone();
            let old_path = spaces.list()[0].path.clone();
            let s = spaces.open(&id).await.unwrap();
            document_service::create_document(&s.db, s.user_id, workspace_id(&s).await, "Keep me", None)
                .await
                .unwrap();

            let target = dir.join("moved").join("personal.db");
            let moved = spaces.move_file(&id, target.clone()).await.unwrap();
            assert_eq!(moved.path, target);
            assert!(old_path.exists(), "old file is kept");

            let reloaded = load(&dir);
            assert_eq!(reloaded.list()[0].path, target);
            let m = reloaded.open(&id).await.unwrap();
            let docs = document_service::list_documents(&m.db, m.user_id, workspace_id(&m).await).await.unwrap();
            assert_eq!(docs[0].title, "Keep me");

            assert!(matches!(spaces.move_file(&id, target).await, Err(AppError::Invalid(_))), "won't overwrite");
        });
    }

    #[test]
    fn the_only_space_cannot_be_removed() {
        tauri::async_runtime::block_on(async {
            let dir = temp_dir();
            let spaces = load(&dir);
            let id = spaces.list()[0].id.clone();
            assert!(matches!(spaces.remove(&id).await, Err(AppError::Invalid(_))));
        });
    }
}
