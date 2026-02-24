use axum::{
    extract::{Path, State, Multipart, Query},
    response::{Html, Redirect, IntoResponse},
    http::{StatusCode, HeaderMap, header},
    body::Body,
};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncReadExt};
use tokio_util::io::ReaderStream;
use uuid::Uuid;
use std::path::PathBuf;
use std::io::SeekFrom;

use crate::state::AppState;
use crate::models::{SearchParams, Video, Person};

// ── Index ─────────────────────────────────────────────────────────────────────

pub async fn index(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let video_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM videos")
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let people_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM people")
        .fetch_one(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let recent_videos = sqlx::query_as::<_, Video>(
        r#"SELECT id, title, filename, date_filmed, place, description,
                  reel_number, digitised_by, duration_secs, file_size,
                  created_at, updated_at
           FROM videos ORDER BY created_at DESC LIMIT 6"#
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("index.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { video_count, people_count, recent_videos })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// ── List / search ─────────────────────────────────────────────────────────────

pub async fn list_videos(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, StatusCode> {
    let videos = search_videos_query(&state, &params).await?;
    let all_people = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at FROM people ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("videos/list.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { videos, all_people, params })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn search_videos_partial(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, StatusCode> {
    let videos = search_videos_query(&state, &params).await?;
    let tmpl = state.templates.get_template("videos/search_results.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { videos })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

async fn search_videos_query(state: &AppState, params: &SearchParams) -> Result<Vec<Video>, StatusCode> {
    let videos = if let Some(ref q) = params.q {
        sqlx::query_as::<_, Video>(
            r#"SELECT id, title, filename, date_filmed, place, description,
                      reel_number, digitised_by, duration_secs, file_size,
                      created_at, updated_at
               FROM videos
               WHERE search_vector @@ plainto_tsquery('english', $1)
               ORDER BY ts_rank(search_vector, plainto_tsquery('english', $1)) DESC, date_filmed NULLS LAST"#
        )
        .bind(q)
        .fetch_all(&state.db)
        .await
    } else if let Some(pid) = params.person_id {
        sqlx::query_as::<_, Video>(
            r#"SELECT v.id, v.title, v.filename, v.date_filmed, v.place, v.description,
                      v.reel_number, v.digitised_by, v.duration_secs, v.file_size,
                      v.created_at, v.updated_at
               FROM videos v
               JOIN video_people vp ON vp.video_id = v.id
               WHERE vp.person_id = $1
               ORDER BY v.date_filmed NULLS LAST"#
        )
        .bind(pid)
        .fetch_all(&state.db)
        .await
    } else if let Some(year) = params.year {
        sqlx::query_as::<_, Video>(
            r#"SELECT id, title, filename, date_filmed, place, description,
                      reel_number, digitised_by, duration_secs, file_size,
                      created_at, updated_at
               FROM videos
               WHERE EXTRACT(YEAR FROM date_filmed) = $1
               ORDER BY date_filmed NULLS LAST"#
        )
        .bind(year as f64)
        .fetch_all(&state.db)
        .await
    } else {
        sqlx::query_as::<_, Video>(
            r#"SELECT id, title, filename, date_filmed, place, description,
                      reel_number, digitised_by, duration_secs, file_size,
                      created_at, updated_at
               FROM videos ORDER BY date_filmed NULLS LAST, created_at DESC"#
        )
        .fetch_all(&state.db)
        .await
    };

    videos.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// ── Upload form ───────────────────────────────────────────────────────────────

pub async fn upload_form(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let all_people = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at FROM people ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("videos/form.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { all_people, video => minijinja::Value::UNDEFINED })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// ── Upload (multipart) ────────────────────────────────────────────────────────

pub async fn upload_video(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Redirect, (StatusCode, String)> {
    let mut title = String::new();
    let mut date_filmed: Option<String> = None;
    let mut place: Option<String> = None;
    let mut description: Option<String> = None;
    let mut reel_number: Option<String> = None;
    let mut digitised_by: Option<String> = None;
    let mut person_ids: Vec<String> = Vec::new();
    let mut saved_filename: Option<String> = None;
    let mut file_size: Option<i64> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                let original_name = field.file_name().unwrap_or("video.mp4").to_string();
                let ext = std::path::Path::new(&original_name)
                    .extension().and_then(|e| e.to_str()).unwrap_or("mp4");
                let unique_name = format!("{}.{}", Uuid::new_v4(), ext);
                let dest = PathBuf::from(&state.videos_dir).join(&unique_name);
                let data = field.bytes().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;
                file_size = Some(data.len() as i64);
                tokio::fs::write(&dest, &data).await.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
                saved_filename = Some(unique_name);
            }
            "title"        => title = field.text().await.unwrap_or_default(),
            "date_filmed"  => date_filmed = Some(field.text().await.unwrap_or_default()),
            "place"        => place = Some(field.text().await.unwrap_or_default()),
            "description"  => description = Some(field.text().await.unwrap_or_default()),
            "reel_number"  => reel_number = Some(field.text().await.unwrap_or_default()),
            "digitised_by" => digitised_by = Some(field.text().await.unwrap_or_default()),
            "person_ids"   => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() { person_ids.push(val); }
            }
            _ => {}
        }
    }

    let filename = saved_filename.ok_or((StatusCode::BAD_REQUEST, "No file uploaded".into()))?;

    let dob = date_filmed
        .filter(|s| !s.is_empty())
        .map(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date".into()))?;

    let row: (Uuid,) = sqlx::query_as(
        r#"INSERT INTO videos (title, filename, date_filmed, place, description, reel_number, digitised_by, file_size)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id"#
    )
    .bind(&title)
    .bind(&filename)
    .bind(dob)
    .bind(place.filter(|s| !s.is_empty()))
    .bind(description.filter(|s| !s.is_empty()))
    .bind(reel_number.filter(|s| !s.is_empty()))
    .bind(digitised_by.filter(|s| !s.is_empty()))
    .bind(file_size)
    .fetch_one(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let video_id = row.0;

    for pid_str in person_ids {
        if let Ok(pid) = Uuid::parse_str(&pid_str) {
            let _ = sqlx::query(
                "INSERT INTO video_people (video_id, person_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
            )
            .bind(video_id)
            .bind(pid)
            .execute(&state.db)
            .await;
        }
    }

    Ok(Redirect::to(&format!("/videos/{}", video_id)))
}

// ── Video detail ──────────────────────────────────────────────────────────────

pub async fn video_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let video = sqlx::query_as::<_, Video>(
        r#"SELECT id, title, filename, date_filmed, place, description,
                  reel_number, digitised_by, duration_secs, file_size,
                  created_at, updated_at
           FROM videos WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let people = sqlx::query_as::<_, Person>(
        r#"SELECT p.id, p.name, p.relationship, p.date_of_birth, p.notes, p.created_at
           FROM people p JOIN video_people vp ON vp.person_id = p.id
           WHERE vp.video_id = $1 ORDER BY p.name"#
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("videos/detail.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { video, people })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// ── Stream video ──────────────────────────────────────────────────────────────

pub async fn stream_video(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    req_headers: HeaderMap,
) -> Result<impl IntoResponse, StatusCode> {
    let row: Option<(String,)> = sqlx::query_as("SELECT filename FROM videos WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let filename = row.ok_or(StatusCode::NOT_FOUND)?.0;
    let path = PathBuf::from(&state.videos_dir).join(&filename);

    let ext = std::path::Path::new(&filename)
        .extension().and_then(|e| e.to_str()).unwrap_or("mp4");
    let content_type = match ext {
        "mp4"  => "video/mp4",
        "webm" => "video/webm",
        "mov"  => "video/quicktime",
        "avi"  => "video/x-msvideo",
        _      => "application/octet-stream",
    };

    let file_size = tokio::fs::metadata(&path).await.map_err(|_| StatusCode::NOT_FOUND)?.len();

    let mut headers = HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, content_type.parse().unwrap());
    headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());

    // Parse Range header: "bytes=<start>-<end>" or "bytes=<start>-"
    if let Some(range_val) = req_headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some(byte_range) = range_val.strip_prefix("bytes=") {
            let mut parts = byte_range.splitn(2, '-');
            let start: Option<u64> = parts.next().and_then(|s| s.parse().ok());
            let end_raw: Option<u64> = parts.next().and_then(|s| s.parse().ok());

            if let Some(start) = start {
                if start >= file_size {
                    return Err(StatusCode::RANGE_NOT_SATISFIABLE);
                }
                let end = end_raw.unwrap_or(file_size - 1).min(file_size - 1);
                let chunk_len = end - start + 1;

                let mut file = File::open(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
                file.seek(SeekFrom::Start(start)).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
                let stream = ReaderStream::new(file.take(chunk_len));
                let body = Body::from_stream(stream);

                headers.insert(
                    header::CONTENT_RANGE,
                    format!("bytes {}-{}/{}", start, end, file_size).parse().unwrap(),
                );
                headers.insert(header::CONTENT_LENGTH, chunk_len.to_string().parse().unwrap());

                return Ok((StatusCode::PARTIAL_CONTENT, headers, body));
            }
        }
    }

    // No Range header — serve full file with Content-Length
    let file = File::open(&path).await.map_err(|_| StatusCode::NOT_FOUND)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    headers.insert(header::CONTENT_LENGTH, file_size.to_string().parse().unwrap());

    Ok((StatusCode::OK, headers, body))
}

// ── Edit video form ───────────────────────────────────────────────────────────

pub async fn edit_video_form(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let video = sqlx::query_as::<_, Video>(
        r#"SELECT id, title, filename, date_filmed, place, description,
                  reel_number, digitised_by, duration_secs, file_size,
                  created_at, updated_at
           FROM videos WHERE id = $1"#
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let all_people = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at FROM people ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // People already tagged on this video
    let tagged_ids: Vec<Uuid> = sqlx::query_as::<_, (Uuid,)>(
        "SELECT person_id FROM video_people WHERE video_id = $1"
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .into_iter()
    .map(|(id,)| id)
    .collect();

    // Convert to strings for easy comparison in template
    let tagged_id_strings: Vec<String> = tagged_ids.iter().map(|u| u.to_string()).collect();

    let tmpl = state.templates.get_template("videos/edit.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { video, all_people, tagged_id_strings })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

// ── Update video metadata ─────────────────────────────────────────────────────

pub async fn update_video(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    mut multipart: Multipart,
) -> Result<Redirect, (StatusCode, String)> {
    let mut title = String::new();
    let mut date_filmed: Option<String> = None;
    let mut place: Option<String> = None;
    let mut description: Option<String> = None;
    let mut reel_number: Option<String> = None;
    let mut digitised_by: Option<String> = None;
    let mut person_ids: Vec<String> = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "title"        => title = field.text().await.unwrap_or_default(),
            "date_filmed"  => date_filmed = Some(field.text().await.unwrap_or_default()),
            "place"        => place = Some(field.text().await.unwrap_or_default()),
            "description"  => description = Some(field.text().await.unwrap_or_default()),
            "reel_number"  => reel_number = Some(field.text().await.unwrap_or_default()),
            "digitised_by" => digitised_by = Some(field.text().await.unwrap_or_default()),
            "person_ids"   => {
                let val = field.text().await.unwrap_or_default();
                if !val.is_empty() { person_ids.push(val); }
            }
            _ => {}
        }
    }

    let dob = date_filmed
        .filter(|s| !s.is_empty())
        .map(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid date".into()))?;

    sqlx::query(
        r#"UPDATE videos SET title=$1, date_filmed=$2, place=$3, description=$4,
           reel_number=$5, digitised_by=$6 WHERE id=$7"#
    )
    .bind(&title)
    .bind(dob)
    .bind(place.filter(|s| !s.is_empty()))
    .bind(description.filter(|s| !s.is_empty()))
    .bind(reel_number.filter(|s| !s.is_empty()))
    .bind(digitised_by.filter(|s| !s.is_empty()))
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Replace all people tags
    sqlx::query("DELETE FROM video_people WHERE video_id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    for pid_str in person_ids {
        if let Ok(pid) = Uuid::parse_str(&pid_str) {
            let _ = sqlx::query(
                "INSERT INTO video_people (video_id, person_id) VALUES ($1, $2) ON CONFLICT DO NOTHING"
            )
            .bind(id)
            .bind(pid)
            .execute(&state.db)
            .await;
        }
    }

    Ok(Redirect::to(&format!("/videos/{}", id)))
}

// ── Delete video ──────────────────────────────────────────────────────────────

pub async fn delete_video(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let row: Option<(String,)> = sqlx::query_as("SELECT filename FROM videos WHERE id = $1")
        .bind(id)
        .fetch_optional(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    sqlx::query("DELETE FROM videos WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if let Some((filename,)) = row {
        let path = PathBuf::from(&state.videos_dir).join(filename);
        let _ = tokio::fs::remove_file(path).await;
    }

    Ok([("HX-Redirect", "/videos")])
}