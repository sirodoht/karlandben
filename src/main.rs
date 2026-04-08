mod app;
mod database;
mod email;
mod handlers;
mod models;
mod services;
mod session;
mod templates;

use email::EmailService;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tracing::info;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .compact()
        .init();

    info!("Starting fogpub server");

    let opts = SqliteConnectOptions::from_str("sqlite://./fogpub.db")
        .expect("failed to parse sqlite url")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(opts)
        .await
        .expect("failed to connect to sqlite");

    info!("Connected to SQLite database");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    info!("Database migrations completed");

    // Email service
    let email_service = EmailService::new();
    if email_service.is_some() {
        info!("Email service configured");
    } else {
        info!("Email service not configured - codes will be printed to stdout");
    }

    let app = app::create_app(pool, email_service);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();
    info!(address = %listener.local_addr().unwrap(), "Server listening");
    axum::serve(listener, app).await.unwrap();
}
