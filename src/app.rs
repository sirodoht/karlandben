use crate::{email::EmailService, handlers};
use axum::{
    Extension, Router,
    routing::{get, post},
};
use sqlx::SqlitePool;

pub fn create_app(pool: SqlitePool, email_service: Option<EmailService>) -> Router {
    Router::new()
        .route("/", get(handlers::index))
        // Auth routes
        .route("/login", get(handlers::login_page).post(handlers::login))
        .route("/verify", post(handlers::verify))
        .route(
            "/profile",
            get(handlers::profile_page).post(handlers::register_name),
        )
        .route("/logout", post(handlers::logout))
        // Add pool to extensions so Session extractor can access it
        .layer(Extension(pool))
        .layer(Extension(email_service))
}
