use sqlx::PgPool;
use minijinja::Environment;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub templates: Arc<Environment<'static>>,
    pub videos_dir: String,
}
