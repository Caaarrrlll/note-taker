use sea_orm::DbErr;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Error type for services and commands.
///
/// Serialised to the frontend as `{ kind, message }` so the UI can react to
/// specific cases (e.g. `unauthenticated` → show a login screen).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] DbErr),
    #[error("{0} not found")]
    NotFound(&'static str),
    #[error("not signed in")]
    Unauthenticated,
    #[error("invalid email or password")]
    InvalidCredentials,
    #[error("{0}")]
    Invalid(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Db(_) => "database",
            AppError::NotFound(_) => "notFound",
            AppError::Unauthenticated => "unauthenticated",
            AppError::InvalidCredentials => "invalidCredentials",
            AppError::Invalid(_) => "invalid",
            AppError::Internal(_) => "internal",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
