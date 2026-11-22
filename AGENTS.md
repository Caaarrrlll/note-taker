# Note Taker: Agent Guide

A Notion-style, block-based note-taking **desktop app**. Built with Tauri v2 (Rust backend, web frontend) and SeaORM.

Storage is a **local SQLite file by default**, with no setup. Users can optionally **connect to an existing PostgreSQL or MySQL/MariaDB server** instead. The app never starts or manages database servers itself. It only connects to them.

The plan and current status are in [ROADMAP.md](ROADMAP.md). Read it before starting work, and update it when you finish a roadmap item.

## Layout

```
package.json             npm scripts + all frontend deps and the Tauri CLI (one `npm install` at the root)
web/                     Frontend: React + TypeScript + BlockNote (Vite root; builds to web/dist)
  index.html, vite.config.ts, tsconfig.json
  src/api.ts             Typed wrappers for every Tauri command (keep in sync with commands/)
  src/App.tsx            Shell: loads workspace + pages, owns selection
  src/components/        Sidebar (page tree), PageView (title), Editor (BlockNote + autosave + / menu)
  src/editorSchema.tsx   BlockNote schema: default blocks + custom blocks (pageLink)
  src/pagesContext.ts    Page list, navigation, trash/restore for components inside the editor
  src/pendingSaves.ts    Flush-on-close registry for debounced saves
Cargo.toml               Cargo workspace (D12): members crates/note-core, apps/desktop; shared dependency versions
crates/note-core/        Core library: everything that touches the database (no Tauri, no HTTP)
  src/error.rs           AppError, serialized to the frontend as { kind, message }
  src/services/          Business logic + queries; generic over `ConnectionTrait`
  src/services/tests.rs  Integration tests (in-memory SQLite by default)
  src/entities/          SeaORM entity models (users, workspaces, documents, blocks)
  src/db/mod.rs          DbConfig (env + SQLite fallback) and connect() (also runs migrations)
  src/db/migrations/     SeaORM migrations, named mYYYYMMDD_NNNNNN_description.rs
apps/desktop/            Desktop app (Tauri shell), depends on note-core
  src/lib.rs             App entry: .env, DB connect, local-user sign-in, app.manage(...), commands
  src/session.rs         Session (signed-in user) — commands take the user from here
  src/commands/          #[tauri::command] fns: State<DatabaseConnection> + State<Session>
  tauri.conf.json        Tauri config (frontendDist = ../../web/dist)
  capabilities/          Tauri v2 permission sets
target/                  Build output for the whole workspace (installers in target/release/bundle/)
Makefile.toml            cargo-make tasks (dev, build, test, clean, ...)
.env.example             Optional local config; copy to .env
```

## Domain model

- **User**: owns workspaces. Local mode uses one passwordless user (`is_local = true`), created on first run and signed in automatically. `auth_service::register`/`login` (argon2) exist for a future login screen but aren't exposed as commands yet.
- **Workspace**: belongs to a user and contains documents.
- **Document**: a page. `parent_id` points at another document to nest pages in a tree. `is_archived` is a soft delete.
- **Block**: one top-level editor block. `content` holds BlockNote's JSON for that block, including nested children. `block_type` mirrors the block's `type`. `sort_order` is its position. The editor saves the whole page through `save_document_blocks`, which diffs against the stored rows.

## Setup and commands

Prerequisites: Rust stable, Node.js, and `cargo install tauri-cli cargo-make`. No database server is needed.

```sh
npm install                       # once (or: cargo make install)
cargo make dev                    # = cargo tauri dev; starts Vite too (local SQLite unless DATABASE_URL is set)
npm run typecheck                 # frontend type check
cargo make bundle                 # = npm run tauri build: installers for this OS (target/release/bundle/)
cargo make test                   # = cargo test --workspace (backend tests on in-memory SQLite)
cargo clippy --workspace --all-targets -- -D warnings
```

The Tauri CLI is also an npm dev dependency (`npm run tauri ...`), pinned to the same minor version as the Rust `tauri` crate.

**CI/CD:** `.github/workflows/ci.yml` runs on pushes to master and on PRs. `.github/workflows/release.yml` runs on `v*` tags: the tag must equal `version` in `tauri.conf.json`, and it builds Windows and Linux installers into a draft release.

Migrations run automatically on connect. There's no separate migrate step.

## Configuration

- Runtime config comes from environment variables, which are loaded from an optional `.env` through `dotenvy`.
- `DATABASE_URL` unset: SQLite at `<app_data_dir>/notes.db`. On Windows that's `%APPDATA%\com.caaarrrlll.notetaker\notes.db`.
- `DATABASE_URL=postgres://…` (or `postgresql://`), `mariadb://…` or `mysql://…` connects to that existing server database. `mariadb://` is rewritten to `mysql://` by `db::driver_url`, because MariaDB uses the MySQL driver. The database must already exist; the app creates the tables.
- When you add a new setting, add it to `.env.example` with a comment explaining it. Never commit `.env`.

## Database portability (important)

Every change must work on **SQLite, PostgreSQL and MySQL/MariaDB**.

- Use the SeaORM / sea-query builders only. Don't write raw SQL or backend-specific expressions (`::jsonb`, `NOW()`, `RETURNING` assumptions) in migrations or services. If backend-specific code is truly needed (for example full-text search), branch on `db.get_database_backend()`.
- Timestamp columns: use `migrations::utc_timestamp(manager, col)` (`DATETIME(6)` on MySQL, timestamptz elsewhere), with `DateTimeUtc` in entities. `created_at` and `updated_at` are set by `touch_timestamps!` in each entity's `before_save`. Don't rely on DB defaults.
- Don't give JSON columns DB defaults; set them in code.
- Give string columns explicit lengths (`string_len`), since MySQL limits indexed `VARCHAR` length.
- `cargo make test` covers SQLite. If a change touches schema or queries, also run the tests with `TEST_DATABASE_URL` pointing at a throwaway Postgres or MySQL database, if one is available.

## Conventions

- **Adding a feature end to end:** migration (if the schema changes), then entity, then service fn (generic over `ConnectionTrait`, taking `user_id` and checking ownership), then command (`State<DatabaseConnection>` + `State<Session>`, user from `session.require_user()`), then register it in `generate_handler!` in `lib.rs`, then a test in `services/tests.rs`, then a wrapper in `web/src/api.ts`.
- **Ownership:** never accept a user/owner ID from the frontend. Load documents with `get_owned_document` and workspaces with `get_owned_workspace`. Another user's data is reported as NotFound.
- **DTOs:** response structs use `#[serde(rename_all = "camelCase")]`.
- **Migrations:** before the first release, existing migrations may still be edited (reset local DBs afterwards). After release, never edit an applied migration. Add a new one and register it in `db/migrations/mod.rs` in order.
- **Errors:** services and commands return `AppResult<T>` (`error.rs`). Don't hide real DB errors as "not found".
- Commands return response DTOs (`*Response`), not raw entity models.
- IDs are UUID v4, generated in Rust.
- Keep commands thin. Logic belongs in `services/`.
- `note-core` must stay free of Tauri types (it doesn't depend on Tauri at all). A self-hosted web server (ROADMAP Phase 7) will reuse them behind HTTP endpoints, so the user ID must always arrive as a parameter.

## Gotchas

- Keep the npm `@tauri-apps/api` minor version in line with the Rust `tauri` crate (both 2.11 now). The Tauri CLI warns about a mismatch.
- Page edits autosave after 600 ms (titles after 400 ms). Anything with a debounced save must use `registerFlusher`/`trackSave` from `web/src/pendingSaves.ts` so it is flushed when the window closes.
- In-memory SQLite (`sqlite::memory:`) is forced to a single connection, because each connection would otherwise get its own empty database.
- The dev environment is Windows. The cargo-make scripts use PowerShell commands.
