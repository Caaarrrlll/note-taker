use std::sync::RwLock;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

/// Who is signed in to this app instance. Managed as Tauri state.
///
/// Commands take the user from here — never from frontend arguments — so the
/// UI can only reach the signed-in user's data. In local mode the local user is
/// signed in at startup; a future login screen will call `sign_in` after
/// `auth_service::login`.
#[derive(Default)]
pub struct Session {
    user_id: RwLock<Option<Uuid>>,
}

impl Session {
    pub fn sign_in(&self, user_id: Uuid) {
        *self.user_id.write().unwrap_or_else(|e| e.into_inner()) = Some(user_id);
    }

    #[allow(dead_code)] // used by the login UI (not built yet)
    pub fn sign_out(&self) {
        *self.user_id.write().unwrap_or_else(|e| e.into_inner()) = None;
    }

    pub fn user_id(&self) -> Option<Uuid> {
        *self.user_id.read().unwrap_or_else(|e| e.into_inner())
    }

    pub fn require_user(&self) -> AppResult<Uuid> {
        self.user_id().ok_or(AppError::Unauthenticated)
    }
}
