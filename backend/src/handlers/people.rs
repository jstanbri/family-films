use axum::{
    extract::{Path, State, Form, Query},
    response::{Html, Redirect, IntoResponse},
    http::StatusCode,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::state::AppState;
use crate::models::{CreatePerson, UpdatePerson, Person};

pub async fn list_people(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let people = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at FROM people ORDER BY name"
    )
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("people/list.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { people })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn new_person_form(
    State(state): State<AppState>,
) -> Result<Html<String>, StatusCode> {
    let tmpl = state.templates.get_template("people/form.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { person => minijinja::Value::UNDEFINED })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn create_person(
    State(state): State<AppState>,
    Form(form): Form<CreatePerson>,
) -> Result<Redirect, StatusCode> {
    let dob = form.date_of_birth
        .filter(|s| !s.is_empty())
        .map(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    sqlx::query(
        "INSERT INTO people (name, relationship, date_of_birth, notes) VALUES ($1, $2, $3, $4)"
    )
    .bind(&form.name)
    .bind(form.relationship.filter(|s| !s.is_empty()))
    .bind(dob)
    .bind(form.notes.filter(|s| !s.is_empty()))
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to("/people"))
}

pub async fn person_detail(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at FROM people WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let videos = sqlx::query_as::<_, crate::models::Video>(
        r#"SELECT v.id, v.title, v.filename, v.date_filmed, v.place, v.description,
                  v.reel_number, v.digitised_by, v.duration_secs, v.file_size,
                  v.created_at, v.updated_at
           FROM videos v
           JOIN video_people vp ON vp.video_id = v.id
           WHERE vp.person_id = $1
           ORDER BY v.date_filmed NULLS LAST"#
    )
    .bind(id)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("people/detail.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { person, videos })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn edit_person_form(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Html<String>, StatusCode> {
    let person = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at FROM people WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .ok_or(StatusCode::NOT_FOUND)?;

    let tmpl = state.templates.get_template("people/form.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { person })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}

pub async fn update_person(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Form(form): Form<UpdatePerson>,
) -> Result<Redirect, StatusCode> {
    let dob = form.date_of_birth
        .filter(|s| !s.is_empty())
        .map(|s| chrono::NaiveDate::parse_from_str(&s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    sqlx::query(
        "UPDATE people SET name=$1, relationship=$2, date_of_birth=$3, notes=$4 WHERE id=$5"
    )
    .bind(&form.name)
    .bind(form.relationship.filter(|s| !s.is_empty()))
    .bind(dob)
    .bind(form.notes.filter(|s| !s.is_empty()))
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Redirect::to(&format!("/people/{}", id)))
}

pub async fn delete_person(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    sqlx::query("DELETE FROM people WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

#[derive(Deserialize)]
pub struct PeopleSearch {
    pub q: Option<String>,
}

pub async fn search_people_partial(
    State(state): State<AppState>,
    Query(params): Query<PeopleSearch>,
) -> Result<Html<String>, StatusCode> {
    let q = params.q.unwrap_or_default();
    let pattern = format!("%{}%", q.to_lowercase());

    let people = sqlx::query_as::<_, Person>(
        "SELECT id, name, relationship, date_of_birth, notes, created_at
         FROM people WHERE lower(name) LIKE $1 ORDER BY name LIMIT 20"
    )
    .bind(pattern)
    .fetch_all(&state.db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let tmpl = state.templates.get_template("people/search_results.html")
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let html = tmpl.render(minijinja::context! { people })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Html(html))
}