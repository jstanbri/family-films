-- Family Films — initial schema
-- sqlx migrate run will apply this automatically on startup

CREATE EXTENSION IF NOT EXISTS "pgcrypto";

-- ── People ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS people (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name          TEXT NOT NULL,
    relationship  TEXT,                  -- e.g. "Father", "Aunt", "Family friend"
    date_of_birth DATE,
    notes         TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_people_name ON people (lower(name));

-- ── Videos ───────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS videos (
    id            UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title         TEXT NOT NULL,
    filename      TEXT NOT NULL UNIQUE,  -- actual file on disk
    date_filmed   DATE,
    place         TEXT,
    description   TEXT,
    reel_number   TEXT,
    digitised_by  TEXT,
    duration_secs INT,                   -- auto-populated if you add ffprobe later
    file_size     BIGINT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_videos_date     ON videos (date_filmed);
CREATE INDEX idx_videos_place    ON videos (lower(place));
CREATE INDEX idx_videos_reel     ON videos (reel_number);
CREATE INDEX idx_videos_title    ON videos (lower(title));

-- Full-text search across title, place, description
ALTER TABLE videos ADD COLUMN IF NOT EXISTS search_vector TSVECTOR
    GENERATED ALWAYS AS (
        to_tsvector('english',
            coalesce(title, '') || ' ' ||
            coalesce(place, '') || ' ' ||
            coalesce(description, '') || ' ' ||
            coalesce(reel_number, '') || ' ' ||
            coalesce(digitised_by, '')
        )
    ) STORED;

CREATE INDEX idx_videos_fts ON videos USING GIN (search_vector);

-- ── Video ↔ People (many-to-many) ────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS video_people (
    video_id   UUID NOT NULL REFERENCES videos(id) ON DELETE CASCADE,
    person_id  UUID NOT NULL REFERENCES people(id) ON DELETE CASCADE,
    PRIMARY KEY (video_id, person_id)
);

CREATE INDEX idx_video_people_person ON video_people (person_id);

-- ── updated_at trigger ────────────────────────────────────────────────────────
CREATE OR REPLACE FUNCTION set_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = NOW();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER videos_updated_at
    BEFORE UPDATE ON videos
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
