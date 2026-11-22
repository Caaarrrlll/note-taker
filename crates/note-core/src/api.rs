//! Response types shared by the desktop commands and the web server, so both
//! send the frontend exactly the same JSON.

use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::entities::{blocks, documents, users, workspaces};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub is_local: bool,
}

impl From<users::Model> for UserResponse {
    fn from(m: users::Model) -> Self {
        Self { id: m.id, email: m.email, display_name: m.display_name, is_local: m.is_local }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
}

impl From<workspaces::Model> for WorkspaceResponse {
    fn from(m: workspaces::Model) -> Self {
        Self { id: m.id, name: m.name }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentResponse {
    pub id: Uuid,
    pub title: String,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub is_archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<documents::Model> for DocumentResponse {
    fn from(m: documents::Model) -> Self {
        Self {
            id: m.id,
            title: m.title,
            workspace_id: m.workspace_id,
            parent_id: m.parent_id,
            is_archived: m.is_archived,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockResponse {
    pub id: Uuid,
    pub block_type: String,
    pub content: serde_json::Value,
}

impl From<blocks::Model> for BlockResponse {
    fn from(m: blocks::Model) -> Self {
        Self { id: m.id, block_type: m.block_type, content: m.content }
    }
}
