# Family Films Archive

A private, self-hosted archive for digitised family cine films.

**Stack:** Rust (Axum) · PostgreSQL · HTMX · Docker Compose

---

## Quick Start

### Prerequisites

- Docker Desktop (or Docker Engine + Compose plugin)
- That's it — Rust compiles inside the container

### 1. Clone / copy this project

```powershell
family-films/
├── docker-compose.yml
├── .env.example
└── backend/
    ├── Dockerfile
    ├── Cargo.toml
    ├── migrations/
    ├── src/
    └── templates/
```

### 2. Configure

```bash
cp .env.example .env
# Edit .env and set a strong POSTGRES_PASSWORD
```

### 3. Start

```bash
docker compose up --build
```

The first build will take a few minutes while Cargo compiles dependencies.
Subsequent builds are fast thanks to Docker layer caching.

Open [localhost](http://localhost:3000)

---

## Usage

### Uploading Films

1. Navigate to **Upload** in the nav bar
2. Choose your mp4/mov file
3. Fill in title, date, place, reel number, digitised-by
4. Tick the people who appear in the film
5. Hit **Upload**

The video file is stored in a Docker named volume (`video_data`) so it persists across container restarts.

### Adding People

Go to **People → Add Person**. Add name, relationship (e.g. *Mother*, *Grandfather*, *Family friend*), date of birth, and any notes.

### Searching

The **Films** page has live search across title, place, description, and reel number (full-text search via PostgreSQL). You can also filter by person or year.

---

## Development (without Docker)

If you want to iterate quickly without rebuilding the container:

```bash
# Terminal 1 — database only
docker compose up db

# Terminal 2 — run the app locally
cd backend
export DATABASE_URL=postgres://films:changeme@localhost:5432/family_films
export VIDEOS_DIR=./videos
cargo run
```

Set `RUST_LOG=family_films=debug` for verbose logging.

---

## Backup

### Database

```bash
docker compose exec db pg_dump -U films family_films > backup_$(date +%Y%m%d).sql
```

### Video files

```bash
# Find the volume mount point
docker volume inspect family-films_video_data
# Or just copy out of the container
docker compose cp backend:/app/videos ./videos-backup
```

### Restore

```bash
cat backup_20240101.sql | docker compose exec -T db psql -U films family_films
```

---

## Hosting on the Web (future)

When you're ready to expose this beyond localhost:

1. Set a strong `POSTGRES_PASSWORD` in `.env`
2. Put a reverse proxy (Caddy or nginx) in front — Caddy gives you automatic HTTPS in two lines
3. Change `ports` in `docker-compose.yml` to only bind to `127.0.0.1:3000` and let the proxy forward
4. Consider adding HTTP Basic Auth in the proxy for a simple access layer

---

## Schema Overview

```powershell

people
  id, name, relationship, date_of_birth, notes, created_at

videos
  id, title, filename, date_filmed, place, description,
  reel_number, digitised_by, duration_secs, file_size,
  created_at, updated_at, search_vector (auto, GIN indexed)

video_people  (many-to-many join)
  video_id → videos.id
  person_id → people.id
```

Migrations are handled automatically by `sqlx::migrate!` on startup.
