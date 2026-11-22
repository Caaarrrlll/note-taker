# Self-hosting the web app

`note-server` serves the Note Taker web app and its API from one container. Your notes live in SQLite under `/data` by default. You can also point the server at an existing PostgreSQL or MariaDB database.

## Quick start (Docker)

```sh
docker run -d \
    --name note-taker \
    -p 8080:8080 \
    -v note-taker-data:/data \
    ghcr.io/caaarrrlll/note-taker:latest
```

Open `http://<server-ip>:8080/` and create the first account. After that, registration is closed unless you set `REGISTRATION=open`.

The session cookie is marked `Secure`, so over plain HTTP the browser won't send it back. To try the app on your LAN without HTTPS, add `-e COOKIE_SECURE=false`. Don't do this for anything exposed beyond your LAN.

## Unraid

1. Copy [`deploy/unraid/note-taker.xml`](../deploy/unraid/note-taker.xml) to `/boot/config/plugins/dockerMan/templates-user/my-note-taker.xml` on the flash drive.
2. Go to **Docker → Add Container** and choose the **note-taker** template.
3. Check the port and the data path (default `/mnt/user/appdata/note-taker`), then apply.

The template runs the container as `99:100` (Unraid's `nobody:users`), so files in appdata get the usual ownership.

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `DATABASE_URL` | empty (SQLite in `/data/notes.db`) | `postgres://USER:PASSWORD@HOST:5432/DB`, `postgresql://…`, `mariadb://USER:PASSWORD@HOST:3306/DB` or `mysql://…`. The database must exist; the tables are created on start. |
| `REGISTRATION` | `closed` | `open` lets anyone who can reach the server create an account. |
| `COOKIE_SECURE` | `true` | Keep `true` behind HTTPS. Set `false` only for plain-HTTP testing. |
| `SESSION_TTL_DAYS` | `30` | How long a sign-in lasts. |
| `PORT` | `8080` | Port inside the container. |
| `DATA_DIR` | `/data` | Where the SQLite database goes. |

## HTTPS with a reverse proxy

Put the container behind the reverse proxy you already run (SWAG, Nginx Proxy Manager, Traefik, Caddy, …) and forward `https://<domain>/` to `http://<server-ip>:8080/`. Nothing else is needed:
- The app uses relative URLs and one origin.
- No WebSockets.
- Request bodies up to 16 MB, which covers a whole page saved in one request. Raise the proxy's body size limit if it's lower.

The server doesn't use `X-Forwarded-*` headers: login limits are per email address, not per IP, and no URLs are built from the request. There's no "trusted proxy" setting to get wrong.

### Network exposure

- **Publish only the reverse proxy's HTTPS port.** The container port (`8080`) only needs to be reachable by the proxy.
- **Keep the database private.** If you use PostgreSQL or MariaDB, its port stays on the server's private network and is never forwarded or published. Only `note-server` connects to it. Give it its own database user, with rights on its own database only.
- **Clients need only HTTPS.** Browsers, and later the desktop app in remote mode, so they work from networks that block everything else, such as office, hotel or VPN networks.

Security notes:
- **Sign-in:** the session cookie is `HttpOnly`, `SameSite=Lax` and `Secure`. Only a SHA-256 hash of each session token is stored.
- **Login attempts:** 10 failed sign-ins for one email address lock it for 15 minutes.
- **Forged requests:** every API call must carry `X-Requested-With: note-taker`, which other sites can't add to cross-site requests. The server never enables CORS.
- **Users:** each user only sees their own pages.

## Backups

- **SQLite:** back up the `/data` folder. For a consistent copy, stop the container first, or copy `notes.db` together with `notes.db-wal`.
- **PostgreSQL / MariaDB:** use that database's usual dump tools (`pg_dump`, `mariadb-dump`).

## Updating

Pull the new image and recreate the container. Database migrations run automatically on start.
