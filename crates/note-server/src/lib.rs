//! Note Taker web server (ROADMAP Phase 7).
//!
//! - `POST /api/<command>` mirrors the desktop app's Tauri commands one-to-one
//!   (same names, same camelCase JSON arguments, same responses), so the
//!   frontend only swaps its transport.
//! - `/api/auth/*` handles first-run setup, login and logout with an HttpOnly
//!   session cookie.
//! - Everything else serves the built web app.
//!
//! CSRF: every `POST /api/...` must carry `X-Requested-With: note-taker`.
//! Browsers can't add that header to cross-site requests without a CORS
//! preflight, which this server never allows; the cookie is also `SameSite=Lax`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration as StdDuration, Instant};

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use sea_orm::DatabaseConnection;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

use note_core::api::{BlockResponse, DocumentResponse, UserResponse, WorkspaceResponse};
use note_core::error::{AppError, AppResult};
use note_core::services::block_service::{self, BlockInput};
use note_core::services::{auth_service, document_service, session_service, workspace_service};

pub const SESSION_COOKIE: &str = "note_session";
pub const CSRF_HEADER: &str = "x-requested-with";
pub const CSRF_VALUE: &str = "note-taker";

/// Failed logins allowed per email address within `LOGIN_WINDOW`.
const MAX_FAILED_LOGINS: u32 = 10;
const LOGIN_WINDOW: StdDuration = StdDuration::from_secs(15 * 60);

/// Largest request body (a whole page is saved in one request).
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;

pub struct Config {
    /// Anyone may create an account (otherwise only the first-run setup can).
    pub registration_open: bool,
    pub session_ttl: chrono::Duration,
    /// Add `Secure` to the cookie. Turn off only for plain-HTTP local testing.
    pub cookie_secure: bool,
    /// Built frontend (`web/dist`); `None` serves the API only.
    pub web_dir: Option<PathBuf>,
}

#[derive(Clone)]
pub struct AppState {
    db: DatabaseConnection,
    config: Arc<Config>,
    failed_logins: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
}

impl AppState {
    pub fn new(db: DatabaseConnection, config: Config) -> Self {
        Self { db, config: Arc::new(config), failed_logins: Arc::default() }
    }
}

pub fn app(state: AppState) -> Router {
    let api = Router::new()
        .route("/auth/status", get(auth_status))
        .route("/auth/setup", post(auth_setup))
        .route("/auth/register", post(auth_register))
        .route("/auth/login", post(auth_login))
        .route("/auth/logout", post(auth_logout))
        .route("/{command}", post(rpc))
        .layer(middleware::from_fn(require_csrf_header))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES));

    let mut router = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .nest("/api", api);

    if let Some(dir) = &state.config.web_dir {
        // Unknown paths get index.html, so reloading the app at any URL works.
        let files = ServeDir::new(dir).fallback(ServeFile::new(dir.join("index.html")));
        router = router.fallback_service(files);
    }

    router.layer(middleware::from_fn(security_headers)).with_state(state)
}

// ------------------------------------------------------------------ errors

struct ApiError(AppError);

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self.0 {
            AppError::NotFound(_) => StatusCode::NOT_FOUND,
            AppError::Unauthenticated | AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::Invalid(_) => StatusCode::BAD_REQUEST,
            AppError::Db(_) | AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            // Log the detail; don't send database internals to the client.
            eprintln!("request failed: {}", self.0);
            let body = json!({ "kind": self.0.kind(), "message": "Something went wrong on the server" });
            return (status, Json(body)).into_response();
        }
        (status, Json(&self.0)).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

fn error_response(status: StatusCode, kind: &str, message: &str) -> Response {
    (status, Json(json!({ "kind": kind, "message": message }))).into_response()
}

// -------------------------------------------------------------- middleware

async fn require_csrf_header(request: Request, next: Next) -> Response {
    let ok = request.method() != Method::POST
        || request.headers().get(CSRF_HEADER).and_then(|v| v.to_str().ok()) == Some(CSRF_VALUE);
    if !ok {
        return error_response(StatusCode::FORBIDDEN, "forbidden", "Missing X-Requested-With header");
    }
    next.run(request).await
}

async fn security_headers(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    headers.insert("x-frame-options", HeaderValue::from_static("DENY"));
    headers.insert("referrer-policy", HeaderValue::from_static("same-origin"));
    response
}

// ----------------------------------------------------------------- sessions

fn session_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, value)| value.to_string())
}

async fn current_user(state: &AppState, headers: &HeaderMap) -> AppResult<Uuid> {
    let token = session_token(headers).ok_or(AppError::Unauthenticated)?;
    session_service::user_for_token(&state.db, &token)
        .await?
        .ok_or(AppError::Unauthenticated)
}

fn session_cookie(state: &AppState, token: &str, max_age_secs: i64) -> HeaderValue {
    let secure = if state.config.cookie_secure { "; Secure" } else { "" };
    let cookie = format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age_secs}{secure}");
    HeaderValue::from_str(&cookie).expect("cookie is ASCII")
}

/// Sign `user_id` in: new session + cookie, and the user's JSON.
async fn signed_in(state: &AppState, user_id: Uuid) -> ApiResult<Response> {
    let token = session_service::create_session(&state.db, user_id, state.config.session_ttl).await?;
    let user = auth_service::get_user(&state.db, user_id).await?;
    let mut response = Json(UserResponse::from(user)).into_response();
    let cookie = session_cookie(state, &token, state.config.session_ttl.num_seconds());
    response.headers_mut().insert(header::SET_COOKIE, cookie);
    Ok(response)
}

// --------------------------------------------------------------------- auth

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuthStatus {
    /// No accounts yet: the first visitor creates one.
    needs_setup: bool,
    registration_open: bool,
    user: Option<UserResponse>,
}

async fn auth_status(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Json<AuthStatus>> {
    let user = match current_user(&state, &headers).await {
        Ok(id) => Some(UserResponse::from(auth_service::get_user(&state.db, id).await?)),
        Err(AppError::Unauthenticated) => None,
        Err(e) => return Err(e.into()),
    };
    Ok(Json(AuthStatus {
        needs_setup: !auth_service::has_accounts(&state.db).await?,
        registration_open: state.config.registration_open,
        user,
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NewAccount {
    email: String,
    display_name: String,
    password: String,
}

async fn create_account(state: &AppState, account: NewAccount) -> ApiResult<Response> {
    let user = auth_service::register(&state.db, &account.email, &account.display_name, &account.password).await?;
    workspace_service::ensure_default_workspace(&state.db, user.id).await?;
    signed_in(state, user.id).await
}

/// First run only: create the first account.
async fn auth_setup(State(state): State<AppState>, Json(account): Json<NewAccount>) -> ApiResult<Response> {
    if auth_service::has_accounts(&state.db).await? {
        return Ok(error_response(StatusCode::FORBIDDEN, "forbidden", "Setup is already done; sign in instead"));
    }
    create_account(&state, account).await
}

async fn auth_register(State(state): State<AppState>, Json(account): Json<NewAccount>) -> ApiResult<Response> {
    if !state.config.registration_open {
        return Ok(error_response(StatusCode::FORBIDDEN, "forbidden", "Registration is closed on this server"));
    }
    create_account(&state, account).await
}

#[derive(Deserialize)]
struct Login {
    email: String,
    password: String,
}

async fn auth_login(State(state): State<AppState>, Json(login): Json<Login>) -> ApiResult<Response> {
    let key = login.email.trim().to_lowercase();
    {
        let mut failed = state.failed_logins.lock().unwrap_or_else(|e| e.into_inner());
        failed.retain(|_, (_, since)| since.elapsed() < LOGIN_WINDOW);
        if failed.get(&key).is_some_and(|(count, _)| *count >= MAX_FAILED_LOGINS) {
            return Ok(error_response(
                StatusCode::TOO_MANY_REQUESTS,
                "rateLimited",
                "Too many failed sign-ins; try again in 15 minutes",
            ));
        }
    }

    match auth_service::login(&state.db, &login.email, &login.password).await {
        Ok(user) => {
            state.failed_logins.lock().unwrap_or_else(|e| e.into_inner()).remove(&key);
            workspace_service::ensure_default_workspace(&state.db, user.id).await?;
            signed_in(&state, user.id).await
        }
        Err(AppError::InvalidCredentials) => {
            let mut failed = state.failed_logins.lock().unwrap_or_else(|e| e.into_inner());
            failed.entry(key).or_insert((0, Instant::now())).0 += 1;
            Err(AppError::InvalidCredentials.into())
        }
        Err(e) => Err(e.into()),
    }
}

async fn auth_logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    if let Some(token) = session_token(&headers) {
        session_service::delete_session(&state.db, &token).await?;
    }
    let mut response = StatusCode::NO_CONTENT.into_response();
    response.headers_mut().insert(header::SET_COOKIE, session_cookie(&state, "", 0));
    Ok(response)
}

// ---------------------------------------------------------------- commands

/// `POST /api/<command>`: the desktop commands over HTTP. Arguments are the
/// same camelCase JSON object the desktop app passes to Tauri (`spaceId` is
/// ignored: on the server, the signed-in account is the space).
async fn rpc(
    State(state): State<AppState>,
    Path(command): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let user_id = current_user(&state, &headers).await?;
    let args: Value = if body.is_empty() {
        json!({})
    } else {
        serde_json::from_slice(&body).map_err(|e| AppError::Invalid(format!("Invalid JSON: {e}")))?
    };
    Ok(Json(dispatch(&state.db, user_id, &command, args).await?))
}

fn args<T: DeserializeOwned>(value: Value) -> AppResult<T> {
    serde_json::from_value(value).map_err(|e| AppError::Invalid(format!("Invalid arguments: {e}")))
}

fn to_json<T: Serialize>(value: T) -> AppResult<Value> {
    serde_json::to_value(value).map_err(|e| AppError::Internal(e.to_string()))
}

fn map_all<M, R: From<M>>(models: Vec<M>) -> Vec<R> {
    models.into_iter().map(R::from).collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct IdArg {
    id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceArg {
    workspace_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NameArg {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateDocumentArgs {
    workspace_id: Uuid,
    title: String,
    parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitleArgs {
    id: Uuid,
    title: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MoveArgs {
    id: Uuid,
    parent_id: Option<Uuid>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentArg {
    document_id: Uuid,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SaveBlocksArgs {
    document_id: Uuid,
    blocks: Vec<BlockInput>,
}

async fn dispatch(db: &DatabaseConnection, user: Uuid, command: &str, a: Value) -> AppResult<Value> {
    use document_service as docs;
    match command {
        "get_current_user" => to_json(UserResponse::from(auth_service::get_user(db, user).await?)),

        "list_workspaces" => {
            to_json(map_all::<_, WorkspaceResponse>(workspace_service::list_user_workspaces(db, user).await?))
        }
        "create_workspace" => {
            let NameArg { name } = args(a)?;
            to_json(WorkspaceResponse::from(workspace_service::create_workspace(db, user, &name).await?))
        }

        "list_documents" => {
            let WorkspaceArg { workspace_id } = args(a)?;
            to_json(map_all::<_, DocumentResponse>(docs::list_documents(db, user, workspace_id).await?))
        }
        "get_document" => {
            let IdArg { id } = args(a)?;
            to_json(DocumentResponse::from(docs::get_owned_document(db, user, id).await?))
        }
        "create_document" => {
            let CreateDocumentArgs { workspace_id, title, parent_id } = args(a)?;
            to_json(DocumentResponse::from(docs::create_document(db, user, workspace_id, &title, parent_id).await?))
        }
        "update_document_title" => {
            let TitleArgs { id, title } = args(a)?;
            to_json(DocumentResponse::from(docs::update_document_title(db, user, id, &title).await?))
        }
        "archive_document" => {
            let IdArg { id } = args(a)?;
            to_json(docs::archive_document(db, user, id).await?)
        }
        "move_document" => {
            let MoveArgs { id, parent_id } = args(a)?;
            to_json(DocumentResponse::from(docs::move_document(db, user, id, parent_id).await?))
        }
        "list_archived_documents" => {
            let WorkspaceArg { workspace_id } = args(a)?;
            to_json(map_all::<_, DocumentResponse>(docs::list_archived_documents(db, user, workspace_id).await?))
        }
        "restore_document" => {
            let IdArg { id } = args(a)?;
            to_json(map_all::<_, DocumentResponse>(docs::restore_document(db, user, id).await?))
        }
        "delete_document_permanently" => {
            let IdArg { id } = args(a)?;
            to_json(docs::delete_document_permanently(db, user, id).await?)
        }

        "get_document_blocks" => {
            let DocumentArg { document_id } = args(a)?;
            to_json(map_all::<_, BlockResponse>(block_service::get_document_blocks(db, user, document_id).await?))
        }
        "save_document_blocks" => {
            let SaveBlocksArgs { document_id, blocks } = args(a)?;
            block_service::save_document_blocks(db, user, document_id, blocks).await?;
            Ok(Value::Null)
        }

        _ => Err(AppError::NotFound("Command")),
    }
}

#[cfg(test)]
mod tests;
