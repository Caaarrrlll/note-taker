//! Users and authentication.
//!
//! Today the app runs in *local mode*: a single passwordless user is created on
//! first run and signed in automatically. `register`/`login` and password
//! hashing are implemented and tested so a login screen can be added later
//! (e.g. for shared server databases) without changing the commands.

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

use crate::entities::users;
use crate::error::{AppError, AppResult};

/// Placeholder address for the local user (`.invalid` is a reserved TLD).
pub const LOCAL_USER_EMAIL: &str = "local@note-taker.invalid";
pub const MIN_PASSWORD_LEN: usize = 8;

/// Return the local user, creating it on first run.
pub async fn ensure_local_user<C: ConnectionTrait>(db: &C) -> AppResult<users::Model> {
    if let Some(user) = users::Entity::find()
        .filter(users::Column::IsLocal.eq(true))
        .one(db)
        .await?
    {
        return Ok(user);
    }

    let user = users::ActiveModel {
        id: Set(Uuid::new_v4()),
        email: Set(LOCAL_USER_EMAIL.into()),
        display_name: Set("Local User".into()),
        password_hash: Set(None),
        is_local: Set(true),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(user)
}

pub async fn get_user<C: ConnectionTrait>(db: &C, id: Uuid) -> AppResult<users::Model> {
    users::Entity::find_by_id(id)
        .one(db)
        .await?
        .ok_or(AppError::NotFound("User"))
}

fn normalize_email(email: &str) -> String {
    email.trim().to_lowercase()
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hashing failed: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok())
        .unwrap_or(false)
}

/// Create a password-protected account.
#[allow(dead_code)] // used by the login UI (not built yet)
pub async fn register<C: ConnectionTrait>(
    db: &C,
    email: &str,
    display_name: &str,
    password: &str,
) -> AppResult<users::Model> {
    let email = normalize_email(email);
    let display_name = display_name.trim();

    if email.len() > 255 || !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(AppError::Invalid("Enter a valid email address".into()));
    }
    if display_name.is_empty() || display_name.chars().count() > 255 {
        return Err(AppError::Invalid("Display name must be 1–255 characters".into()));
    }
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(AppError::Invalid(format!(
            "Password must be at least {MIN_PASSWORD_LEN} characters"
        )));
    }
    if users::Entity::find()
        .filter(users::Column::Email.eq(&email))
        .one(db)
        .await?
        .is_some()
    {
        return Err(AppError::Invalid("An account with this email already exists".into()));
    }

    let user = users::ActiveModel {
        id: Set(Uuid::new_v4()),
        email: Set(email),
        display_name: Set(display_name.to_string()),
        password_hash: Set(Some(hash_password(password)?)),
        is_local: Set(false),
        ..Default::default()
    }
    .insert(db)
    .await?;
    Ok(user)
}

/// Check credentials. The local user can never log in with a password.
#[allow(dead_code)] // used by the login UI (not built yet)
pub async fn login<C: ConnectionTrait>(db: &C, email: &str, password: &str) -> AppResult<users::Model> {
    let user = users::Entity::find()
        .filter(users::Column::Email.eq(normalize_email(email)))
        .filter(users::Column::IsLocal.eq(false))
        .one(db)
        .await?;

    match user {
        Some(u) if u.password_hash.as_deref().is_some_and(|h| verify_password(password, h)) => Ok(u),
        _ => Err(AppError::InvalidCredentials),
    }
}
