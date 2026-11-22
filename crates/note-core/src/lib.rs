//! Note Taker core library: everything that touches the database.
//!
//! Shared by the desktop app (`apps/desktop`) and, later, the web server and the
//! transfer tool (ROADMAP D12). Keep it free of Tauri and HTTP types: callers
//! pass in the database connection and the signed-in user's ID.

pub mod db;
pub mod entities;
pub mod error;
pub mod services;
