# test-instance API (Rust)

Rust backend and small web app for projects, Hackatime, Lapse, and Hack Club Attend. PostgreSQL is the source of truth; Redis is deliberately used for short-lived cache entries, OAuth state, rate limits, idempotency responses, and distributed write locks so provider calls and repeat reads do not overload the database.

## Run locally

```bash
cp .env.example .env
# Set APP_ENCRYPTION_KEY to a real 32-byte hex key.
docker compose up -d
cargo run
```

The server runs migrations from `database/migrations/` at startup and exposes `GET /healthz`. The migration ledger lives in PostgreSQL (`_sqlx_migrations`), so migrations are applied exactly once. PostgREST, if you add it, should point at this same migrated PostgreSQL database; it does not execute migration files itself.

## Current API

The web app is served by the Rust process. Its flow is intentionally small:

1. `/` asks only for an email address.
2. `/sign-in` offers only **Sign in with Hack Club**.
3. Hack Club OAuth redirects to `/dashboard`, where users can create projects, connect Hackatime, choose linked Hackatime projects, and see per-project plus total tracked time.

The API uses the `session` HttpOnly cookie issued after Hack Club OAuth. It does not accept browser-supplied user IDs or provider tokens.

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/auth/hackclub/login?email=…` | Starts Hack Club OAuth |
| `GET` | `/auth/hackatime/login` | Starts Hackatime OAuth for the signed-in user |
| `GET` | `/api/v1/me` | Returns the session user |
| `GET` / `POST` | `/api/v1/projects` | Lists project time / creates a project |
| `GET` | `/api/v1/hackatime/projects` | Lists available Hackatime projects |
| `PUT` | `/api/v1/projects/:project_id/hackatime-projects` | Links Hackatime project names (owner only) |
| `GET` | `/api/v1/projects/:project_id/hackatime` | Fetches linked Hackatime project summaries |
| `GET` | `/api/v1/projects/:project_id/lapses` | Fetches Lapse timelapses matching linked names |
| `POST` | `/api/v1/attendance/events/:event_id/register` | Registers current user with Attend |

`POST /attendance` accepts an optional `Idempotency-Key` header. Its registration row has a database uniqueness constraint as the final duplicate-prevention layer; Redis makes repeated requests cheap and provider calls serialized.

## OAuth setup

Create one Hack Club Auth app and one Hackatime OAuth app. Register these local callback URLs exactly (or their HTTPS production equivalents):

```text
http://localhost:3000/auth/hackclub/callback
http://localhost:3000/auth/hackatime/callback
```

Copy the client IDs, secrets, and callback URLs into `.env`. Set `COOKIE_SECURE=true` when deploying on HTTPS.
