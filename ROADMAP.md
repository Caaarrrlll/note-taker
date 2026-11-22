# Roadmap

A Notion-style, block-based note-taking app (Tauri v2 + Rust + SeaORM): a desktop app today, with a self-hosted web version planned.

**Direction:** a personal tool for now, possibly opened up for others once it's stable. Users choose where their notes live: a **local SQLite file** (no setup needed) or a **server database** (PostgreSQL or MySQL/MariaDB). A **transfer tool** moves an existing data set between backends, for example from local SQLite to Postgres later on. **Spaces** (e.g. Work, Personal, School) keep separate sets of notes, each with its own storage: Work could stay in a local SQLite file while Personal lives on a server database. A **self-hosted web version** (one container, e.g. on an Unraid server) serves the same app at a web URL, with real login. The desktop app can also use that server for a space (**remote mode**), so a space can follow you to any device that can reach a normal HTTPS website, for example a work laptop where only local storage is allowed for work notes but personal notes still live on your own server.

Status key: `[x]` done · `[~]` partial · `[ ]` not started

---

## Decisions

| # | Question | Decision / options | Notes |
|---|---|---|---|
| D1 | Database | **Decided:** local SQLite by default, with a connector for an existing PostgreSQL or MySQL/MariaDB server. The app never starts DB servers. | Done in Phase 1. |
| D2 | Auth / users | **Decided:** one automatic local user for the desktop app. Login scaffolding (session, argon2, register/login services) is built and tested, but has no UI yet. | The web version (Phase 7) requires real login. |
| D3 | Frontend stack | **Decided:** React + TypeScript + Vite | |
| D4 | Editor | **Decided:** BlockNote (core is MPL-2.0. Its GPL-3.0 `xl-*` packages are compatible now that the app is GPL-3.0.) | |
| D5 | Block storage | **Decided:** one row per top-level block holding BlockNote's JSON. The whole page is saved at once and diffed against stored rows. | |
| D6 | Shared server DB security | **Leaning:** the Phase 7 web server *is* the API in front of the DB | If several people point the desktop app directly at one DB, each client holds DB credentials and could read everyone's data. With the web server, only the server holds the credentials and users sign in. The desktop app could later connect to that server too (see D8). |
| D7 | Web API style | Open: JSON-over-HTTP that mirrors the Tauri commands (recommended) · GraphQL · tRPC-style | Mirroring the commands keeps one service layer and lets `src/api.ts` switch transport (Tauri `invoke` vs `fetch`) with no UI changes. |
| D8 | Desktop ↔ web server | **Decided (2026-11-22):** remote mode. A desktop space can point at a self-hosted `note-server` over HTTPS instead of at a database. | One set of notes on every device without sync. Works from restricted networks (work laptops with their own VPN, hotel and office Wi-Fi), where database ports are normally blocked and only HTTPS gets out. The device never holds database credentials, only a revocable token (D6). |
| D11 | Space storage kinds | **Decided (2026-11-22):** `sqlite` (local file) · `remote` (HTTPS URL of a `note-server` + device token) · direct server DB (`postgres://`, `mysql://`) kept for development and trusted local networks only | `remote` is the normal way to put a space on a server. A direct DB connection means the client holds DB credentials and the database port has to be reachable, so it's not offered for use over the internet. |
| D12 | Repository layout | **Decided (2026-11-22):** one monorepo for the frontend and the "middleware" (desktop shell, core library, web server, tools) | The desktop app, the web server and the transfer tool share the same entities, migrations and services, and the same React frontend. One repo keeps them on one version and one CI. Layout: see Phase 7, "Architecture". |
| D9 | What a space maps to | **Leaning:** a space = one storage connection + one workspace in it | A space's notes live wholly in its own storage, so Work and Personal can use different databases. Several spaces may share one server database as separate workspaces. The space list itself is local app config, because it has to exist before any database is opened. |
| D10 | Space encryption | **Leaning:** optional end-to-end encryption per space, done in the app (not by the database) | Encrypting in the app protects the data on every backend (SQLite file, Postgres, MariaDB) and against anyone who can read the database, including a server admin or a stolen backup. SQLCipher would only cover local SQLite files. Cost: no server-side search, and a lost passphrase means lost notes. |

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
- [ ] Connection settings screen in the app. Superseded by per-space storage settings (Phase 6).

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
- [ ] Settings screen: per-space storage (see Phase 6)
- [ ] Frontend tests (Vitest + mocked `invoke`)

## Phase 5: Quality of life

- [ ] Full-text search. Needs a separate implementation per backend: Postgres `tsvector`, SQLite FTS5, MySQL `FULLTEXT`.
- [ ] Drag and drop for blocks and pages
- [x] Trash panel: restore, or delete forever (two-step). The sidebar trash icon moves a page to the trash in one click.
- [ ] Keyboard shortcuts
- [ ] Light and dark theme
- [ ] Markdown import and export

## Phase 6: Spaces (Work / Personal / School)

A level above pages: separate **spaces** such as Work, Personal and School, each with its own page tree, trash and **storage setting**. Example: Work stays in a local SQLite file on this machine, while Personal lives on a server database (Postgres/MariaDB on Unraid) so it is available from other machines.

"Sync" here means the space *lives on the server*: its notes are stored there, not in a local copy. A server space uses **remote mode** (D8, D11): the app talks HTTPS to a self-hosted `note-server` (Phase 7), never to the database itself. An unreachable server makes that space unavailable until it is back (see "Offline" below).

**Storage is chosen per space, independently.** Any mix is allowed: space X on the server, space Z local only, and each space can be switched later (see "Moving a space between storages"). A local-only space never contacts the server, not even for metadata, and the space list itself (`spaces.json`) stays on the device.

Typical setup: a **Work** space in a local SQLite file on the work laptop (work notes never leave it), and a **Personal** space in remote mode for things learned along the way, reachable from the work laptop, a home PC and the browser alike.

**Model** (D9)
- [ ] Space list in a local config file (`spaces.json` in the app config dir): ID, name, colour/icon, storage kind (D11: `sqlite` with a file path, `remote` with a server URL, or a direct DB URL for development), last-opened page
- [ ] Secrets in the OS keychain (Windows Credential Manager / Secret Service / macOS Keychain), not in `spaces.json`: the device token of a `remote` space, or the password of a direct DB connection
- [ ] First run: one "Personal" space on the default local SQLite file. Existing installs migrate to that space without moving data.
- [ ] `DATABASE_URL` keeps working as a dev/CI override for the default space

**Backend**
- [ ] Hold one connection pool per open local or direct-DB space (opened lazily, closed when a space is removed). Migrations run per connection, as today.
- [ ] `remote` spaces hold an HTTP client instead of a pool. Each command is forwarded to the server's matching JSON endpoint (Phase 7, D7) with the device token; the server runs the same services. Needs Phase 7 first.
- [ ] Commands take a `space_id`; services stay unchanged (they already take a connection and `user_id`)
- [ ] Each space's database keeps its own local user and workspace, created on first connect, as today
- [ ] "Test connection" command for the settings screen
- [ ] Tests: two spaces on different SQLite files stay isolated

**Frontend**
- [ ] Space switcher at the top of the sidebar (name + colour), with keyboard shortcut
- [ ] Space settings: rename, colour, storage (local file, or remote: server URL + sign in to get a device token; the direct DB form only in a developer section), test connection, remove space (data is never deleted by removing a space)
- [ ] Local SQLite location per space: pick where the space's database file lives (file picker via the Tauri dialog plugin). Options: create a new file in a chosen folder, open an existing `notes.db`, or move the current file to a new folder (copy, verify, switch; the old file is kept until the user deletes it). The path is stored in `spaces.json`; default stays `<app_data_dir>/<space>.db`. Warn when the folder is cloud-synced (OneDrive, Dropbox, Google Drive): opening the same file from two machines at once can corrupt it.
- [ ] Clear status when a server space is unreachable (with retry), without blocking the other spaces
- [ ] Remember the last open page per space

**Moving a space between storages**
- [ ] "Move to server" / "Move to local file" in space settings, using the Phase 2 transfer tool's library code
- [ ] Copy first, verify row counts, then switch the space over; the old storage is kept until the user deletes it

**Encryption** (optional, per space; D10)

Goal: notes stay unreadable to anyone who gets at the database itself (a copied SQLite file, a leaked backup, someone with access to the Unraid server or the DB credentials). Data is encrypted and decrypted only inside the app.

- [ ] Per-space passphrase. Key derivation with Argon2id (already a dependency) into a key-encryption key.
- [ ] A random data key per space, stored in the database only in wrapped (encrypted) form. Changing the passphrase re-wraps the data key without re-encrypting every note.
- [ ] Authenticated encryption (XChaCha20-Poly1305 or AES-256-GCM) with a fresh random nonce per value. Encrypt page titles and block content; also bind each value to its row ID, so ciphertext can't be swapped between rows unnoticed.
- [ ] Schema: an encryption marker + wrapped key + passphrase check value on the workspace. Titles and block content need columns that fit ciphertext (titles outgrow `VARCHAR(255)`, content becomes an encrypted blob instead of JSON). Needs a new migration, portable across all three backends.
- [ ] Unlock flow: ask for the passphrase when the space is opened. Optionally remember the key in the OS keychain. Lock the space again on demand or after idle time.
- [ ] Recovery key shown once when encryption is turned on. Without the passphrase or recovery key, the notes cannot be recovered, by design. The UI must say this plainly.
- [ ] Turn encryption on or off for an existing space by re-writing its data (reuses the "move space" copy-verify-switch steps).
- [ ] Search in encrypted spaces runs locally over decrypted pages only. The Phase 5 database full-text search can't index encrypted content.
- [ ] Web version (Phase 7): to stay end-to-end, encryption has to run in the browser (WebCrypto), so the server never sees keys or plaintext.
- [ ] Documented limits: structure and metadata stay visible (number of pages, nesting, sizes, timestamps). Encryption protects data at rest, not a machine where the space is unlocked.
- [ ] Tests: round trip, wrong passphrase, tampered ciphertext is rejected, passphrase change, and no plaintext title or content in the raw database rows.

**Offline** (later, only if needed)
- [ ] Read-only local cache of a server space for when the server is unreachable (on a plane, or a network that blocks the server). Most useful for remote spaces on a travelling laptop. Real two-way sync with conflict handling is a much bigger feature and stays out of scope until there is a clear need.

## Phase 7: Self-hosted web app (container)

Run the same app in a browser, served from one container on a home server (e.g. **Unraid**) at a web URL. The React frontend already runs in a browser. What's missing is an HTTP server around the existing Rust services, and real authentication.

**Architecture**
- [ ] Monorepo (D12): one repository, one Cargo workspace plus the frontend package. Proposed layout:

  ```
  note-taker/
  ├── Cargo.toml             workspace: members = crates/*, apps/desktop
  ├── crates/
  │   ├── note-core/         entities, migrations, services, errors (no Tauri, no HTTP)
  │   ├── note-server/       axum web server: JSON API + serves the built frontend
  │   └── note-transfer/     Phase 2 CLI
  ├── apps/desktop/          Tauri shell (today's src-tauri): commands call note-core, or forward
  │                          to note-server for remote spaces
  ├── web/                   React + Vite frontend (today's src/), built once for both targets
  ├── migrations/            (or inside note-core)
  └── Dockerfile             builds web/ + note-server into one image
  ```

  Move in small steps that each build and pass the tests: create the workspace and `note-core` first (Phase 2 needs it), then move `src-tauri` and `src`, then add `note-server`. The Tauri app, web server and transfer tool all depend on `note-core`.

  - [x] Step 1: Cargo workspace at the repo root + `crates/note-core` (entities, migrations, services, errors, tests). `src-tauri` depends on it; build output moved to the root `target/`.
  - [x] Step 2: move `src-tauri` to `apps/desktop` and `src` to `web/` (Vite root is `web/`, one `package.json` at the repo root)
  - [ ] Step 3: add `crates/note-server`
- [ ] Shared API types: request and response structs live in `note-core` (or a small `note-api` crate), used by the server and by the desktop's remote client, so both sides can't drift. Generate the TypeScript types for `web/` from them (e.g. `ts-rs` or `specta`) instead of writing them twice.
- [ ] Web server binary (`note-server`, using axum): JSON endpoints mirroring the Tauri commands (D7), serving the built frontend as static files on the same port
- [ ] `src/api.ts`: choose the transport at build time (`invoke` in the desktop app, `fetch` in the web build). Components stay unchanged.
- [ ] Web build of the frontend: no Tauri-only APIs in the browser (e.g. the window-close hook becomes `beforeunload` / `visibilitychange` + `navigator.sendBeacon`)
- [ ] Health endpoint (`/healthz`) for container health checks

**Authentication** (required: the app will be reachable over the network)
- [ ] Login and register screens, using the existing `auth_service` (argon2)
- [ ] Sessions: random session IDs in a DB `sessions` table, sent as `HttpOnly`, `Secure`, `SameSite=Lax` cookies, with expiry and logout
- [ ] Per-request user: replace the app-wide `Session` with a user from the session cookie. Services already take `user_id` and check ownership.
- [ ] First-run setup: the first visitor creates the admin account. After that, registration is closed by default (`REGISTRATION=closed|invite|open`).
- [ ] **Device tokens** for desktop clients in remote mode (D8): signing in from the desktop app creates a long, random token stored hashed in a `device_tokens` table (user, device name, created, last used), sent as `Authorization: Bearer`. The app keeps it in the OS keychain. A "Devices" page lists them and revokes one without touching the others (e.g. a lost or returned work laptop). Browser sessions keep using cookies.
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
- [ ] Docs: running behind a reverse proxy with HTTPS (Caddy / SWAG / Nginx Proxy Manager / Traefik), and trusting `X-Forwarded-*` headers only from the proxy
- [ ] Docs, network exposure: publish **only** the proxy's HTTPS port. The database stays on the server's private network and its port is never forwarded or published; only `note-server` connects to it, with its own database user. Clients (browser and desktop remote mode) need nothing but HTTPS, so they work from networks that block everything else.
- [ ] Docs: backups (copy the SQLite file, or use a Postgres/MariaDB container on the same server via `DATABASE_URL`)

**Tests**
- [ ] HTTP API integration tests (auth required, ownership enforced, cookie flags, device token create/use/revoke)
- [ ] Desktop remote-mode tests against a test server: open, edit and save a page in a remote space; a revoked token gives a clear "signed out" state
- [ ] CI: build the container image on every PR, and smoke-test `/healthz` and login

## Phase 8: Going public

- [ ] Resolve D6 (shared DB security) and D2 (auth). Phase 7 covers most of this.
- [x] LICENSE: GPL-3.0-or-later
- [ ] README, screenshots and a contributing guide
- [x] App icon set (source: `src-tauri/icons/app-icon.svg`, regenerate with `cargo make icons`)
- [x] Bundling: Windows `.msi` + NSIS `.exe` (verified locally), Linux `.deb` + `.rpm` + AppImage (built in CI)
- [x] Release workflow (`.github/workflows/release.yml`): push a `v*` tag to build Windows and Linux installers into a draft GitHub release. v0.1.0 built successfully.
- [x] Bundle identifier changed to `com.caaarrrlll.notetaker` (the old `.app` ending clashed with macOS bundles)
**Release builds (GitHub Actions)**

Every release should offer an installer *and* a standalone (portable) build for each platform, published automatically by `release.yml`.

- [x] Installers: Windows `.msi` + NSIS `.exe`, Linux `.deb` + `.rpm`
- [x] Linux standalone: AppImage (runs without installing)
- [ ] Windows standalone: portable `.zip` with `note-taker.exe` (tauri-action `uploadPlainBinary`, or zip it in a workflow step). Needs WebView2, which Windows 10/11 already ship.
- [ ] Portable mode: when a `data/` folder (or `portable` marker file) sits next to the executable, keep `notes.db` and settings there instead of the app data dir, so a USB stick or synced folder carries the notes along. Document this in the README.
- [ ] Linux standalone tarball (`.tar.gz` with the binary) for distros where AppImage is awkward. Optional; only if asked for.
- [ ] Clear asset names in the release (`NoteTaker-<version>-windows-x64-setup.exe`, `...-windows-x64-portable.zip`, `...-linux-x64.AppImage`, ...) and a short "which file do I want?" note in the release body
- [ ] SHA-256 checksums file attached to each release
- [ ] Publish step: releases stay drafts until reviewed. Optionally auto-publish once a smoke test passes (installer installs, portable build starts).
- [ ] macOS: `.dmg` installer and a standalone `.app` in a `.zip` (both need signing and notarization, see below)
- [ ] Web build (Phase 7): container image on GHCR, published from the same tag
- [ ] Code signing (Windows SmartScreen warns on unsigned installers)
- [ ] macOS signing and notarization, so macOS builds open without warnings
- [ ] Auto-update (Tauri updater plugin + signed update manifests)
- [ ] Single version source (`tauri.conf.json`, `package.json` and `Cargo.toml` are bumped by hand today)

## Later / stretch

- [ ] Images and file attachments
- [ ] Links between pages and backlinks
- [ ] Sync across devices
