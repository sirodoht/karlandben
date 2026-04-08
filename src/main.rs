use askama::Template;
use axum::{response::Html, routing::get, Router};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;


#[derive(Template)]
#[template(path = "index.html")]
#[allow(dead_code)]
struct IndexTemplate {
    message: &'static str,
}

async fn index() -> Html<String> {
    let tmpl = IndexTemplate {
        message: "hello",
    };
    Html(tmpl.render().unwrap())
}

#[tokio::main]
async fn main() {
    let opts = SqliteConnectOptions::from_str("sqlite://./fogpub.db")
        .expect("failed to parse sqlite url")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .connect_with(opts)
        .await
        .expect("failed to connect to sqlite");

    sqlx::migrate!()
        .run(&pool)
        .await
        .expect("failed to run migrations");

    let app = Router::new().route("/", get(index));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Listening on http://{}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}
