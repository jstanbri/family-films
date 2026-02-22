use chrono::{NaiveDate, DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── Person ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Person {
    pub id: Uuid,
    pub name: String,
    pub relationship: Option<String>,
    pub date_of_birth: Option<NaiveDate>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePerson {
    pub name: String,
    pub relationship: Option<String>,
    pub date_of_birth: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePerson {
    pub name: String,
    pub relationship: Option<String>,
    pub date_of_birth: Option<String>,
    pub notes: Option<String>,
}

// ── Video ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Video {
    pub id: Uuid,
    pub title: String,
    pub filename: String,
    pub date_filmed: Option<NaiveDate>,
    pub place: Option<String>,
    pub description: Option<String>,
    pub reel_number: Option<String>,
    pub digitised_by: Option<String>,
    pub duration_secs: Option<i32>,
    pub file_size: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDetail {
    #[serde(flatten)]
    pub video: Video,
    pub people: Vec<Person>,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub person_id: Option<Uuid>,
    pub place: Option<String>,
    pub year: Option<i32>,
}