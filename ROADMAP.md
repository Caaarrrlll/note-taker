# Roadmap

A Notion-style, block-based note-taking app (Tauri v2 + Rust + SeaORM): a desktop app today, with a self-hosted web version planned.

**Direction:** a personal tool for now, possibly opened up for others once it's stable. Users choose where their notes live: a **local SQLite file** (no setup needed) or a **server database** (PostgreSQL or MySQL/MariaDB). A **transfer tool** moves an existing data set between backends, for example from local SQLite to Postgres later on. A **self-hosted web version** (one container, e.g. on an Unraid server) serves the same app at a web URL, with real login.

Status key: `[x]` done · `[~]` partial · `[ ]` not started

---

## Decisions

| # | Question | Decision / options | Notes |
|---|---|---|---|
| D1 | Database | **Decided:** local SQLite by default, with a connector for an existing PostgreSQL or MySQL/MariaDB server. The app never starts DB servers. | Done in Phase 1. |
| D2 | Auth / users | **Decided:** one automatic local user for the desktop app. Login scaffolding (session, argon2, register/login services) is built and tested, but has no UI yet. | The web version (Phase 6) requires real login. |
| D3 | Frontend stack | **Decided:** React + TypeScript + Vite | |
| D4 | Editor | **Decided:** BlockNote (core is MPL-2.0. Its GPL-3.0 `xl-*` packages are compatible now that the app is GPL-3.0.) | |
| D5 | Block storage | **Decided:** one row per top-level block holding BlockNote's JSON. The whole page is saved at once and diffed against stored rows. | |
| D6 | Shared server DB security | **Leaning:** the Phase 6 web server *is* the API in front of the DB | If several people point the desktop app directly at one DB, each client holds DB credentials and could read everyone's data. With the web server, only the server holds the credentials and users sign in. The desktop app could later connect to that server too (see D8). |
| D7 | Web API style | Open: JSON-over-HTTP that mirrors the Tauri commands (recommended) · GraphQL · tRPC-style | Mirroring the commands keeps one service layer and lets `src/api.ts` switch transport (Tauri `invoke` vs `fetch`) with no UI changes. |
| D8 | Desktop ↔ web server | Open: desktop stays local-only · desktop can also sign in to a self-hosted server ("remote mode") | Remote mode would give one set of notes on every device without sync. Decide after Phase 6 works. |

---

## Phase 0: Foundations

- [x] Tauri v2 project skeleton
- [x] SeaORM entities and migrations: users, workspaces, documents, blocks (Postgres only so far)
- [x] Migrations run on startup
- [x] Document commands: create, get, list, rename, archive
- [x] Workspace commands: create, list
- [x] `.env` configuration (`DATABASE_URL`) through dotenvy, plus `.env.example`
- [x] Agent docs (AGENTS.md / CLAUDE.md) and this roadmap
- [x] D1 decided

## Phase 1: Multi-database support

The same code and migrations run on SQLite, PostgreSQL and MySQL/MariaDB. The app **does not start or manage database servers**. It defaults to a local SQLite file and can connect to an existing server database the user provides.

**Configuration**
- [x] Enable the SeaORM features `sqlx-sqlite`, `sqlx-postgres` and `sqlx-mysql`
- [x] Choose the backend from the `DATABASE_URL` scheme: `sqlite://`, `postgres://`, `mysql://` (MariaDB uses `mysql://`)
- [x] If `DATABASE_URL` is unset, default to SQLite at `<app_data_dir>/notes.db`, with no config needed
- [x] Optional `DATABASE_MAX_CONNECTIONS`
- [x] Pass the DB connection through Tauri managed state instead of a global (makes services testable)
- [ ] Connection settings screen in the app: choose SQLite or a server, enter the connection details, test the connection, and save. Needs the Phase 4 frontend. Store the settings in the app config dir. Ask the user about storing the password, and consider the OS keychain.

**Schema portability**
- [x] Removed the Postgres-only `'{}'::jsonb` default. JSON is always set in code.
- [x] Timestamps are `DateTimeUtc`, set in code by a `before_save` hook (`created_at` on insert, `updated_at` on every save)
- [x] Timestamp columns use `DATETIME(6)` on MySQL/MariaDB (avoids the 2038 overflow of `TIMESTAMP` and keeps microseconds) and timestamptz elsewhere
- [x] UUIDs round-trip on all backends (Postgres `uuid`, MySQL `binary(16)`, SQLite blob)
- [x] Explicit string lengths on every string column
- [x] Rule: no raw, backend-specific SQL (documented in AGENTS.md)

**Testing**
- [x] Service tests run on in-memory SQLite by default (`cargo make test`)
- [x] `TEST_DATABASE_URL` runs the same tests against any existing Postgres or MySQL/MariaDB database
- [x] Verified in CI: all tests pass on SQLite, PostgreSQL 17, MySQL 8.4 and MariaDB 11.4
- [x] CI workflow (`.github/workflows/ci.yml`): frontend build, clippy, SQLite tests, plus PostgreSQL, MySQL and MariaDB tests in CI service containers. Passing.

## Phase 2: Database transfer tool

Move all data from one backend to another, for example from a local SQLite file to Postgres or MariaDB on a server.

**Design**
- Move entities and migrations into a shared crate (for example `crates/note-db`) so the app and the tool use exactly the same schema. This turns the repo into a Cargo workspace.
- CLI binary: `note-transfer --from <URL> --to <URL> [--dry-run] [--batch-size N] [--force]`
- Steps:
  1. Connect to both. Run all migrations on the target, and refuse if the source is on an older schema version than the tool.
  2. Check that the target is empty. Abort unless `--force` is given.
  3. Copy in foreign-key order: users, workspaces, documents, blocks. For documents, insert with `parent_id = NULL` first, then set parents in a second pass, so the self-reference never fails.
  4. Insert in batches inside one target transaction, and roll everything back on any error.
  5. Verify that row counts match per table, plus a checksum of IDs. Print a summary.
- Never modify the source. Recommend a backup first (copying the SQLite file is enough).

**Tasks**
- [ ] Convert the repo to a Cargo workspace and extract the `note-db` crate
- [ ] `note-transfer` CLI (clap) with the steps above
- [ ] `--dry-run`: report counts without writing
- [ ] Tests: SQLite to SQLite in the default suite. SQLite to Postgres and SQLite to MySQL when `TEST_DATABASE_URL` is set, and in CI.
- [ ] Later: in-app "Move my data" screen calling the same library code
- [ ] Docs: a "Moving from SQLite to a server DB" guide in the README

## Phase 3: Usable backend

- [x] Local user created on first run and signed in automatically (Session state)
- [x] Login scaffolding: `Session`, argon2 password hashing, `register`/`login` services with tests
- [ ] Login screen + `login`/`register`/`logout` commands (when needed, see D6)
- [x] Default workspace ("My Notes") on first run
- [x] Block commands: `get_document_blocks`, `save_document_blocks` (one transaction, diffs against stored rows, keeps `created_at`)
- [x] Page tree: `list_documents` returns `parentId`, and the frontend builds the tree
- [x] `move_document` with cycle and same-workspace checks (no UI yet)
- [x] Archive cascades to nested pages
- [x] Trash backend: `list_archived_documents`, `restore_document` (with nested pages; restores to top level if the parent is still trashed), `delete_document_permanently` (trashed pages only)
- [x] Set `updated_at` on every update
- [x] Fix `get_document` so it no longer hides DB errors as "not found"
- [x] Typed `AppError`, sent to the frontend as `{ kind, message }`
- [x] Ownership checks on every command (user comes from the session, never from the frontend)

## Phase 4: Frontend MVP

- [x] Vite + React + TypeScript, wired into Tauri (`devUrl`, `beforeDevCommand`, `frontendDist`)
- [x] `@tauri-apps/api` with typed wrappers (`src/api.ts`)
- [x] App shell: sidebar page tree and editor pane, light/dark following the OS
- [x] BlockNote editor: loads and saves through the backend
- [x] Autosave with debounce, saves run in order, "Saving… / Saved" indicator
- [x] Create, rename, delete (archive) and nest pages from the sidebar
- [x] Block types: everything BlockNote ships (headings, lists, to-dos, quote, code, tables, ...) via the `/` menu and markdown shortcuts
- [x] "Page" item in the `/` menu: creates a nested page and inserts a live link block to it (`pageLink`)
- [x] Deleting a `pageLink` block moves its subpage to the trash. Undo, or pasting the link back, restores it.
- [x] Pending saves (content and title) finish before the window closes (`onCloseRequested` + `pendingSaves.ts`, 3 s timeout)
- [ ] Move pages in the sidebar (drag and drop, uses `move_document`)
- [ ] Settings screen: database connection (see Phase 1)
- [ ] Frontend tests (Vitest + mocked `invoke`)

## Phase 5: Quality of life

- [ ] Full-text search. Needs a separate implementation per backend: Postgres `tsvector`, SQLite FTS5, MySQL `FULLTEXT`.
- [ ] Drag and drop for blocks and pages
- [x] Trash panel: restore, or delete forever (two-step). The sidebar trash icon moves a page to the trash in one click.
- [ ] Keyboard shortcuts
- [ ] Light and dark theme
- [ ] Markdown import and export

## Phase 6: Self-hosted web app (container)

Run the same app in a browser, served from one container on a home server (e.g. **Unraid**) at a web URL. The React frontend already runs in a browser. What's missing is an HTTP server around the existing Rust services, and real authentication.

**Architecture**
- [ ] Cargo workspace: move entities, migrations and services into a shared core crate (also needed by the Phase 2 transfer tool). The Tauri app, web server and transfer tool all depend on it.
- [ ] Web server binary (`note-server`, using axum): JSON endpoints mirroring the Tauri commands (D7), serving the built frontend as static files on the same port
- [ ] `src/api.ts`: choose the transport at build time (`invoke` in the desktop app, `fetch` in the web build). Components stay unchanged.
- [ ] Web build of the frontend: no Tauri-only APIs in the browser (e.g. the window-close hook becomes `beforeunload` / `visibilitychange` + `navigator.sendBeacon`)
- [ ] Health endpoint (`/healthz`) for container health checks

**Authentication** (required: the app will be reachable over the network)
- [ ] Login and register screens, using the existing `auth_service` (argon2)
- [ ] Sessions: random session IDs in a DB `sessions` table, sent as `HttpOnly`, `Secure`, `SameSite=Lax` cookies, with expiry and logout
- [ ] Per-request user: replace the app-wide `Session` with a user from the session cookie. Services already take `user_id` and check ownership.
- [ ] First-run setup: the first visitor creates the admin account. After that, registration is closed by default (`REGISTRATION=closed|invite|open`).
- [ ] Rate-limit login attempts. Constant-time handling of unknown emails.
- [ ] CSRF protection for state-changing requests (SameSite cookies + an origin check)
- [ ] Change password; an admin can reset passwords and disable users
- [ ] Later: optional OIDC / reverse-proxy auth (Authelia, Authentik), common on Unraid setups

**Container**
- [ ] Multi-stage `Dockerfile`: Node builds the frontend, Rust builds `note-server`, and a slim runtime image (non-root user) contains just the binary and static files
- [ ] Configuration via environment variables: `DATABASE_URL` (defaults to SQLite at `/data/notes.db`), `PORT`, `PUBLIC_URL`, `REGISTRATION`, `SESSION_TTL`
- [ ] `/data` volume for the SQLite database (and attachments later)
- [ ] GitHub Actions: build and push the image to GHCR (`ghcr.io/caaarrrlll/note-taker`) on version tags, for `linux/amd64` (plus `arm64` if cheap)
- [ ] Unraid Community Applications template (XML): port, `/mnt/user/appdata/note-taker` → `/data`, environment variables
- [ ] Docs: running behind a reverse proxy with HTTPS (SWAG / Nginx Proxy Manager / Traefik), and trusting `X-Forwarded-*` headers only from the proxy
- [ ] Docs: backups (copy the SQLite file, or use a Postgres/MariaDB container on the same server via `DATABASE_URL`)

**Tests**
- [ ] HTTP API integration tests (auth required, ownership enforced, cookie flags)
- [ ] CI: build the container image on every PR, and smoke-test `/healthz` and login

## Phase 7: Going public

- [ ] Resolve D6 (shared DB security) and D2 (auth). Phase 6 covers most of this.
- [x] LICENSE: GPL-3.0-or-later
- [ ] README, screenshots and a contributing guide
- [x] App icon set (source: `src-tauri/icons/app-icon.svg`, regenerate with `cargo make icons`)
- [x] Bundling: Windows `.msi` + NSIS `.exe` (verified locally), Linux `.deb` + `.rpm` + AppImage (built in CI)
- [x] Release workflow (`.github/workflows/release.yml`): push a `v*` tag to build Windows and Linux installers into a draft GitHub release. v0.1.0 built successfully.
- [x] Bundle identifier changed to `com.caaarrrlll.notetaker` (the old `.app` ending clashed with macOS bundles)
- [ ] Code signing (Windows SmartScreen warns on unsigned installers)
- [ ] macOS builds (`.dmg`), which need signing and notarization to open without warnings
- [ ] Auto-update (Tauri updater plugin + signed update manifests)
- [ ] Single version source (`tauri.conf.json`, `package.json` and `Cargo.toml` are bumped by hand today)

## Later / stretch

- [ ] Images and file attachments
- [ ] Links between pages and backlinks
- [ ] Multiple workspaces in the UI
- [ ] Sync across devices
