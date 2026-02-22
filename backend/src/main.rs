mod db;
mod handlers;
mod models;
mod state;

use axum::{
    routing::{delete, get},
    Router,
    extract::DefaultBodyLimit,
};
use minijinja::Environment;
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "family_films=debug,tower_http=debug".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let videos_dir = std::env::var("VIDEOS_DIR")
        .unwrap_or_else(|_| "./videos".to_string());
    let bind_addr = std::env::var("BIND_ADDR")
        .unwrap_or_else(|_| "0.0.0.0:3000".to_string());

    tokio::fs::create_dir_all(&videos_dir).await?;

    let db = db::create_pool(&database_url).await?;
    db::run_migrations(&db).await?;
    tracing::info!("Database migrations applied");

    let mut env = Environment::new();
    env.set_loader(minijinja::path_loader("templates"));
    env.add_filter("truncate", |s: String, len: usize| -> String {
        if s.chars().count() <= len {
            s
        } else {
            let truncated: String = s.chars().take(len).collect();
            format!("{}…", truncated.trim_end())
        }
    });

    let state = AppState {
        db,
        templates: Arc::new(env),
        videos_dir,
    };

    let app = Router::new()
        .route("/",                         get(handlers::videos::index))
        .route("/videos",          get(handlers::videos::list_videos))
        .route("/videos/search",   get(handlers::videos::search_videos_partial))
        .route("/videos/upload",   get(handlers::videos::upload_form)
                                       .post(handlers::videos::upload_video)
                                       .layer(DefaultBodyLimit::disable()))
        .route("/videos/:id",      get(handlers::videos::video_detail).delete(handlers::videos::delete_video))
        .route("/videos/:id/edit", get(handlers::videos::edit_video_form)
                                       .post(handlers::videos::update_video)
                                       .layer(DefaultBodyLimit::disable()))
        .route("/videos/:id/stream", get(handlers::videos::stream_video))
        .route("/people",          get(handlers::people::list_people))
        .route("/people/new",      get(handlers::people::new_person_form).post(handlers::people::create_person))
        .route("/people/search",   get(handlers::people::search_people_partial))
        .route("/people/:id",      get(handlers::people::person_detail).delete(handlers::people::delete_person))
        .route("/people/:id/edit", get(handlers::people::edit_person_form).post(handlers::people::update_person))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("Listening on http://{}", bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}