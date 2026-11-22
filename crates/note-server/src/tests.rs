use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use super::*;
use note_core::db::{self, DbConfig};

async fn test_app(registration_open: bool) -> Router {
    let db = db::connect(&DbConfig { url: "sqlite::memory:".into(), max_connections: None })
        .await
        .expect("in-memory database");
    let config = Config {
        registration_open,
        session_ttl: chrono::Duration::days(1),
        cookie_secure: true,
        web_dir: None,
    };
    app(AppState::new(db, config))
}

struct Reply {
    status: StatusCode,
    cookie: Option<String>,
    body: Value,
}

/// POST (or GET when `body` is None) with the CSRF header and an optional session cookie.
async fn call(app: &Router, uri: &str, body: Option<Value>, cookie: Option<&str>) -> Reply {
    let mut req = Request::builder()
        .uri(uri)
        .method(if body.is_some() { "POST" } else { "GET" })
        .header(CSRF_HEADER, CSRF_VALUE)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(c) = cookie {
        req = req.header(header::COOKIE, format!("{SESSION_COOKIE}={c}"));
    }
    let req = req.body(Body::from(body.map(|b| b.to_string()).unwrap_or_default())).unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let cookie = res
        .headers()
        .get(header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    Reply { status, cookie, body }
}

/// The session token from a Set-Cookie header.
fn token(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .and_then(|kv| kv.split_once('='))
        .map(|(_, v)| v.to_string())
        .unwrap()
}

fn account(email: &str) -> Value {
    json!({ "email": email, "displayName": "Alice", "password": "correct horse battery" })
}

/// First-run setup; returns the session token.
async fn setup(app: &Router, email: &str) -> String {
    let r = call(app, "/api/auth/setup", Some(account(email)), None).await;
    assert_eq!(r.status, StatusCode::OK, "{:?}", r.body);
    token(&r.cookie.expect("setup signs in"))
}

#[tokio::test]
async fn health_check_answers() {
    let app = test_app(false).await;
    let res = app
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
}

#[tokio::test]
async fn web_app_is_served_with_index_fallback_for_app_urls() {
    let dir = std::env::temp_dir().join(format!("note-server-web-{}", Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("index.html"), "<!doctype html><title>app</title>").unwrap();
    let db = db::connect(&DbConfig { url: "sqlite::memory:".into(), max_connections: None }).await.unwrap();
    let config = Config {
        registration_open: false,
        session_ttl: chrono::Duration::days(1),
        cookie_secure: true,
        web_dir: Some(dir),
    };
    let app = app(AppState::new(db, config));
    for uri in ["/", "/some/page"] {
        let res = app.clone().oneshot(Request::get(uri).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK, "{uri}");
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("<title>app</title>"), "{uri}");
    }
}

#[tokio::test]
async fn first_run_setup_creates_one_account_with_a_safe_cookie() {
    let app = test_app(false).await;
    let status = call(&app, "/api/auth/status", None, None).await;
    assert_eq!(status.body["needsSetup"], true);
    assert_eq!(status.body["user"], Value::Null);

    let r = call(&app, "/api/auth/setup", Some(account("alice@example.com")), None).await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.body["email"], "alice@example.com");
    let cookie = r.cookie.unwrap();
    for part in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/"] {
        assert!(cookie.contains(part), "cookie has {part}: {cookie}");
    }

    let again = call(&app, "/api/auth/setup", Some(account("mallory@example.com")), None).await;
    assert_eq!(again.status, StatusCode::FORBIDDEN, "setup only works once");

    let status = call(&app, "/api/auth/status", None, Some(&token(&cookie))).await;
    assert_eq!(status.body["needsSetup"], false);
    assert_eq!(status.body["user"]["email"], "alice@example.com");
}

#[tokio::test]
async fn requests_without_the_csrf_header_are_rejected() {
    let app = test_app(false).await;
    let req = Request::post("/api/auth/setup")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(account("a@example.com").to_string()))
        .unwrap();
    let res = app.clone().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    let status = call(&app, "/api/auth/status", None, None).await;
    assert_eq!(status.body["needsSetup"], true, "nothing was created");
}

#[tokio::test]
async fn commands_need_a_valid_session() {
    let app = test_app(false).await;
    setup(&app, "alice@example.com").await;
    assert_eq!(call(&app, "/api/list_workspaces", Some(json!({})), None).await.status, StatusCode::UNAUTHORIZED);
    let r = call(&app, "/api/list_workspaces", Some(json!({})), Some("forged")).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.body["kind"], "unauthenticated");
}

#[tokio::test]
async fn notes_round_trip_through_the_command_api() {
    let app = test_app(false).await;
    let t = setup(&app, "alice@example.com").await;

    let ws = call(&app, "/api/list_workspaces", Some(json!({ "spaceId": "ignored" })), Some(&t)).await;
    assert_eq!(ws.status, StatusCode::OK);
    let workspace_id = ws.body[0]["id"].as_str().unwrap().to_string();

    let doc = call(
        &app,
        "/api/create_document",
        Some(json!({ "workspaceId": workspace_id, "title": "From the web", "parentId": null })),
        Some(&t),
    )
    .await;
    assert_eq!(doc.status, StatusCode::OK, "{:?}", doc.body);
    let doc_id = doc.body["id"].as_str().unwrap().to_string();
    assert_eq!(doc.body["workspaceId"], workspace_id, "same camelCase JSON as the desktop app");

    let content = json!({ "type": "paragraph", "content": [{ "type": "text", "text": "hello" }] });
    let saved = call(
        &app,
        "/api/save_document_blocks",
        Some(json!({ "documentId": doc_id, "blocks": [{ "blockType": "paragraph", "content": content }] })),
        Some(&t),
    )
    .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.body);

    let blocks = call(&app, "/api/get_document_blocks", Some(json!({ "documentId": doc_id })), Some(&t)).await;
    assert_eq!(blocks.body[0]["content"], content);

    let list = call(&app, "/api/list_documents", Some(json!({ "workspaceId": workspace_id })), Some(&t)).await;
    assert_eq!(list.body.as_array().unwrap().len(), 1);

    let missing = call(&app, "/api/no_such_command", Some(json!({})), Some(&t)).await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    let bad = call(&app, "/api/get_document", Some(json!({ "id": "not-a-uuid" })), Some(&t)).await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn registration_is_closed_unless_enabled() {
    let closed = test_app(false).await;
    setup(&closed, "admin@example.com").await;
    let r = call(&closed, "/api/auth/register", Some(account("bob@example.com")), None).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    let open = test_app(true).await;
    setup(&open, "admin@example.com").await;
    let r = call(&open, "/api/auth/register", Some(account("bob@example.com")), None).await;
    assert_eq!(r.status, StatusCode::OK);
}

#[tokio::test]
async fn users_cannot_see_each_others_pages() {
    let app = test_app(true).await;
    let alice = setup(&app, "alice@example.com").await;
    let bob_reply = call(&app, "/api/auth/register", Some(account("bob@example.com")), None).await;
    let bob = token(&bob_reply.cookie.unwrap());

    let ws = call(&app, "/api/list_workspaces", Some(json!({})), Some(&alice)).await;
    let doc = call(
        &app,
        "/api/create_document",
        Some(json!({ "workspaceId": ws.body[0]["id"], "title": "Alice's diary", "parentId": null })),
        Some(&alice),
    )
    .await;

    let peek = call(&app, "/api/get_document", Some(json!({ "id": doc.body["id"] })), Some(&bob)).await;
    assert_eq!(peek.status, StatusCode::NOT_FOUND);
    let list = call(&app, "/api/list_documents", Some(json!({ "workspaceId": ws.body[0]["id"] })), Some(&bob)).await;
    assert_eq!(list.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn login_logout_and_rate_limiting() {
    let app = test_app(false).await;
    setup(&app, "alice@example.com").await;

    let wrong = json!({ "email": "alice@example.com", "password": "nope" });
    let r = call(&app, "/api/auth/login", Some(wrong.clone()), None).await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    assert_eq!(r.body["kind"], "invalidCredentials");

    let right = json!({ "email": "Alice@Example.com", "password": "correct horse battery" });
    let r = call(&app, "/api/auth/login", Some(right.clone()), None).await;
    assert_eq!(r.status, StatusCode::OK);
    let t = token(&r.cookie.unwrap());

    let out = call(&app, "/api/auth/logout", Some(json!({})), Some(&t)).await;
    assert_eq!(out.status, StatusCode::NO_CONTENT);
    assert!(out.cookie.unwrap().contains("Max-Age=0"));
    let after = call(&app, "/api/list_workspaces", Some(json!({})), Some(&t)).await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED, "the old cookie no longer works");

    for _ in 0..MAX_FAILED_LOGINS {
        call(&app, "/api/auth/login", Some(wrong.clone()), None).await;
    }
    let locked = call(&app, "/api/auth/login", Some(right), None).await;
    assert_eq!(locked.status, StatusCode::TOO_MANY_REQUESTS, "even the right password waits");
}
