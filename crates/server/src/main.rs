mod auth;
mod config;
mod db;
mod sync;
mod web;

use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;

pub struct AppState {
    pub config: config::ServerConfig,
    pub db: db::Database,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "santui_server=info,tower_http=info".into()),
        )
        .init();

    let config = config::ServerConfig::load();

    tracing::info!("data dir: {:?}", config.data_dir);
    tracing::info!(
        "jwt secret: {}...",
        &config.jwt_secret[..8.min(config.jwt_secret.len())]
    );

    let db = db::Database::open(&config.data_dir).unwrap_or_else(|e| {
        tracing::error!("failed to open database: {e}");
        std::process::exit(1);
    });

    let state = Arc::new(AppState {
        config: config.clone(),
        db,
    });

    let app = Router::new()
        .route("/auth/login", post(auth::post_login))
        .route("/auth/me", get(auth::me))
        .route(
            "/api/v1/data/{plugin}",
            get(sync::get_values).post(sync::upsert_values),
        )
        .route("/api/v1/data/{plugin}/{key}", delete(sync::delete_value))
        .route("/", get(web::dashboard))
        .route("/login", get(web::login_page))
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    tracing::info!("santui-server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("failed to bind to {addr}: {e}");
            std::process::exit(1);
        });

    axum::serve(listener, app).await.unwrap_or_else(|e| {
        tracing::error!("server error: {e}");
        std::process::exit(1);
    });
}
