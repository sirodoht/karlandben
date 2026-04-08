mod app;
mod database;
mod email;
mod handlers;
mod models;
mod services;
mod session;
mod templates;

use clap::Parser;
use email::EmailService;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use tracing::info;

#[derive(Parser)]
#[command(name = "fogpub")]
#[command(about = "A social network for vouching for people you've lived with")]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "3000")]
    port: u16,

    /// Path to SQLite database file
    #[arg(short, long, default_value = "./fogpub.db")]
    database: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_target(true)
        .with_thread_ids(true)
        .with_line_number(true)
        .compact()
        .init();

    info!(port = args.port, database = %args.database, "Starting fogpub server");

    let db_url = format!("sqlite://{}", args.database);
    let opts = SqliteConnectOptions::from_str(&db_url)
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

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", args.port))
        .await
        .unwrap();
    info!(address = %listener.local_addr().unwrap(), "Server listening");
    axum::serve(listener, app).await.unwrap();
}
