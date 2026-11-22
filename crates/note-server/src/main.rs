//! `note-server`: run the Note Taker web server.
//!
//! Configuration (environment variables):
//!   DATA_DIR             folder for the default SQLite database   (default: ./data)
//!   DATABASE_URL         postgres://, mariadb://, mysql:// or sqlite:// (default: SQLite in DATA_DIR)
//!   PORT                 HTTP port                                 (default: 8080)
//!   WEB_DIR              built frontend to serve                   (default: ./web/dist)
//!   REGISTRATION         `closed` or `open`                        (default: closed)
//!   SESSION_TTL_DAYS     how long a sign-in lasts                  (default: 30)
//!   COOKIE_SECURE        `false` only for plain-HTTP local testing (default: true)
//!
//! `note-server healthcheck` exits 0 when the server on PORT answers /healthz
//! (used by the container health check).

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::time::Duration;

use note_core::db::{self, DbConfig};
use note_server::{app, AppState, Config};

fn env_or(name: &str, default: &str) -> String {
    match std::env::var(name) {
        Ok(v) if !v.trim().is_empty() => v.trim().to_string(),
        _ => default.to_string(),
    }
}

fn port() -> Result<u16, String> {
    env_or("PORT", "8080").parse().map_err(|_| "PORT must be a number".to_string())
}

fn healthcheck() -> i32 {
    let check = || -> std::io::Result<bool> {
        let addr = SocketAddr::from(([127, 0, 0, 1], port().unwrap_or(8080)));
        let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(3))?;
        stream.set_read_timeout(Some(Duration::from_secs(3)))?;
        stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
        let mut reply = String::new();
        stream.read_to_string(&mut reply)?;
        Ok(reply.starts_with("HTTP/1.1 200"))
    };
    match check() {
        Ok(true) => 0,
        _ => 1,
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

async fn run() -> Result<(), String> {
    let data_dir = PathBuf::from(env_or("DATA_DIR", "./data"));
    std::fs::create_dir_all(&data_dir).map_err(|e| format!("Cannot create {}: {e}", data_dir.display()))?;

    let db_config = DbConfig::from_env(&data_dir)?;
    let backend = db::backend_name(&db_config.url);
    let conn = db::connect(&db_config)
        .await
        .map_err(|e| format!("Cannot connect to the {backend} database: {e}"))?;

    let web_dir = PathBuf::from(env_or("WEB_DIR", "./web/dist"));
    let ttl_days: i64 = env_or("SESSION_TTL_DAYS", "30")
        .parse()
        .map_err(|_| "SESSION_TTL_DAYS must be a number".to_string())?;
    let config = Config {
        registration_open: env_or("REGISTRATION", "closed").eq_ignore_ascii_case("open"),
        session_ttl: chrono::Duration::days(ttl_days),
        cookie_secure: !env_or("COOKIE_SECURE", "true").eq_ignore_ascii_case("false"),
        web_dir: web_dir.join("index.html").exists().then_some(web_dir.clone()),
    };

    println!("note-server: {backend} database");
    match &config.web_dir {
        Some(dir) => println!("note-server: serving web app from {}", dir.display()),
        None => println!("note-server: no web app at {} (API only)", web_dir.display()),
    }
    if !config.cookie_secure {
        println!("note-server: WARNING cookies are not Secure (COOKIE_SECURE=false); use only on localhost");
    }

    let addr = SocketAddr::from(([0, 0, 0, 0], port()?));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Cannot listen on {addr}: {e}"))?;
    println!("note-server: listening on {addr}");

    axum::serve(listener, app(AppState::new(conn, config)))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .map_err(|e| e.to_string())
}

#[tokio::main]
async fn main() {
    if std::env::args().nth(1).as_deref() == Some("healthcheck") {
        std::process::exit(healthcheck());
    }
    if let Err(e) = run().await {
        eprintln!("note-server: {e}");
        std::process::exit(1);
    }
}
