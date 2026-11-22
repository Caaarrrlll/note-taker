//! Login sessions for the web server (cookie-based).
//!
//! The browser gets a random token; the database only stores its SHA-256
//! hash. Expired sessions are treated as missing and cleaned up.

use argon2::password_hash::rand_core::{OsRng, RngCore};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter, Set};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::entities::sessions;
use crate::error::AppResult;

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_token(token: &str) -> String {
    to_hex(&Sha256::digest(token.as_bytes()))
}

/// Start a session for `user_id`; returns the token to put in the cookie.
pub async fn create_session<C: ConnectionTrait>(db: &C, user_id: Uuid, ttl: Duration) -> AppResult<String> {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let token = to_hex(&bytes);
    let now = Utc::now();

    sessions::ActiveModel {
        token_hash: Set(hash_token(&token)),
        user_id: Set(user_id),
        created_at: Set(now),
        expires_at: Set(now + ttl),
    }
    .insert(db)
    .await?;

    // Housekeeping: drop sessions that have already expired.
    sessions::Entity::delete_many()
        .filter(sessions::Column::ExpiresAt.lt(now))
        .exec(db)
        .await?;
    Ok(token)
}

/// The user a token belongs to, if the session exists and hasn't expired.
pub async fn user_for_token<C: ConnectionTrait>(db: &C, token: &str) -> AppResult<Option<Uuid>> {
    let Some(session) = sessions::Entity::find_by_id(hash_token(token)).one(db).await? else {
        return Ok(None);
    };
    if session.expires_at <= Utc::now() {
        sessions::Entity::delete_by_id(session.token_hash).exec(db).await?;
        return Ok(None);
    }
    Ok(Some(session.user_id))
}

/// Log out: forget this session.
pub async fn delete_session<C: ConnectionTrait>(db: &C, token: &str) -> AppResult<()> {
    sessions::Entity::delete_by_id(hash_token(token)).exec(db).await?;
    Ok(())
}
